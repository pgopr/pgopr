/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS of THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use crate::crd::v1::{PgOprStatus, pgopr};
use crate::manager::ResourceManager;
use crate::{Error, persistent, primary, replica, services};
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{PersistentVolume, PersistentVolumeClaim, Pod, Service};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::{Api, Client, ResourceExt, api::ListParams};
use std::sync::Arc;

/// Cluster represents the desired state of a PostgreSQL Star Configuration
pub struct Cluster {
    manager: ResourceManager,
}

impl Cluster {
    pub fn new(client: Client) -> Self {
        Self {
            manager: ResourceManager::new(client),
        }
    }

    /// Observes the actual state of the cluster and returns a status object
    pub async fn observe(&self, pgopr: &pgopr) -> Result<PgOprStatus, Error> {
        let name = pgopr.name_any();
        let namespace = pgopr.namespace().unwrap_or_else(|| "default".to_string());
        let deploy_api: Api<Deployment> = Api::namespaced(self.manager.get_client(), &namespace);

        let mut status = PgOprStatus {
            phase: "Pending".to_string(),
            ..Default::default()
        };
        let mut failure_reason: Option<String> = self.pod_failure_reason(&namespace, &name).await?;

        if let Ok(primary) = deploy_api.get(&name).await
            && let Some(s) = primary.status
        {
            status.primary_ready = s.ready_replicas.unwrap_or(0) > 0;
        }

        let replicas_spec = pgopr.spec.replicas.unwrap_or(0);
        let mut ready_count = 0;
        for i in 1..=replicas_spec {
            let replica_name = format!("{}-replica-{}", name, i);
            if failure_reason.is_none() {
                failure_reason = self.pod_failure_reason(&namespace, &replica_name).await?;
            }
            if let Ok(replica) = deploy_api.get(&replica_name).await
                && let Some(s) = replica.status
                && s.ready_replicas.unwrap_or(0) > 0
            {
                ready_count += 1;
            }
        }
        status.ready_replicas = ready_count as u32;

        if let Some(reason) = failure_reason {
            status.phase = "Failed".to_string();
            status.conditions = Some(vec![condition(
                pgopr,
                "Ready",
                "False",
                "PodFailure",
                format!("A PostgreSQL pod is not ready: {}", reason),
            )]);
        } else if status.primary_ready && status.ready_replicas == replicas_spec {
            status.phase = "Running".to_string();
            status.conditions = Some(vec![condition(
                pgopr,
                "Ready",
                "True",
                "ClusterReady",
                "All PostgreSQL deployments are ready".to_string(),
            )]);
        } else if status.primary_ready {
            status.phase = "Degraded".to_string();
            status.conditions = Some(vec![condition(
                pgopr,
                "Ready",
                "False",
                "ReplicasNotReady",
                "Primary is ready but one or more replicas are not ready".to_string(),
            )]);
        } else {
            status.conditions = Some(vec![condition(
                pgopr,
                "Ready",
                "False",
                "PrimaryNotReady",
                "Primary deployment is not ready".to_string(),
            )]);
        }

        Ok(status)
    }

    /// Gets the first waiting reason for pods that belong to a deployment.
    ///
    /// # Arguments
    /// - `namespace` - Namespace where the pods reside.
    /// - `app` - Value of the app label used by the deployment.
    async fn pod_failure_reason(
        &self,
        namespace: &str,
        app: &str,
    ) -> Result<Option<String>, Error> {
        let pod_api: Api<Pod> = Api::namespaced(self.manager.get_client(), namespace);

        let selector = format!("app={app}");

        let pods = pod_api
            .list(&ListParams::default().labels(&selector))
            .await?;

        for pod in pods {
            if let Some(status) = pod.status {
                let mut all_statuses = Vec::new();

                if let Some(container_statuses) = status.container_statuses {
                    all_statuses.extend(container_statuses);
                }

                if let Some(init_container_statuses) = status.init_container_statuses {
                    all_statuses.extend(init_container_statuses);
                }

                for container_status in all_statuses {
                    if let Some(state) = container_status.state {
                        let reason = if let Some(waiting) = state.waiting {
                            waiting.reason
                        } else if let Some(terminated) = state.terminated {
                            terminated.reason
                        } else {
                            None
                        };

                        if let Some(reason) = reason
                            && is_failure_reason(&reason)
                        {
                            return Ok(Some(reason));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Ensures all resources for the cluster exist and match the spec.
    ///
    /// # Arguments
    /// - `pgopr` - The PgOpr resource that defines the desired cluster state.
    pub async fn sync(&self, pgopr: Arc<pgopr>) -> Result<(), Error> {
        let name = pgopr.name_any();
        let namespace = pgopr.namespace().unwrap_or_else(|| "default".to_string());
        let storage = pgopr.spec.storage;
        let replicas = pgopr.spec.replicas.unwrap_or(0);

        let pv_name = format!("{}-pv-volume", name);
        let pv = persistent::build_pv(&pv_name, storage, &name, "/tmp/kind", &name);
        self.manager.sync_cluster(pv).await?;

        let pvc_name = format!("{}-pv-claim", name);
        let pvc = persistent::build_pvc(&pvc_name, &namespace, storage, &name);
        self.manager.sync(&pgopr, pvc).await?;

        let primary_deploy = primary::build(&name, &namespace);
        self.manager.sync(&pgopr, primary_deploy).await?;

        let primary_svc = services::build(&name, &namespace);
        self.manager.sync(&pgopr, primary_svc).await?;

        for i in 1..=replicas {
            let replica_name = format!("{}-replica-{}", name, i);
            let slot_name = format!("replica{}", i);

            let replica_pv_name = format!("{}-pv-volume", replica_name);
            let replica_host_path = format!("/tmp/kind-replica-{}", i);
            let replica_pv = persistent::build_pv(
                &replica_pv_name,
                storage,
                &replica_name,
                &replica_host_path,
                &name,
            );
            self.manager.sync_cluster(replica_pv).await?;

            let replica_pvc_name = format!("{}-pv-claim", replica_name);
            let replica_pvc =
                persistent::build_pvc(&replica_pvc_name, &namespace, storage, &replica_name);
            self.manager.sync(&pgopr, replica_pvc).await?;

            let replica_deploy = replica::build(&replica_name, &name, &namespace, &slot_name);
            self.manager.sync(&pgopr, replica_deploy).await?;

            let replica_svc = services::build(&replica_name, &namespace);
            self.manager.sync(&pgopr, replica_svc).await?;
        }

        self.cleanup_stale_replicas(&name, &namespace, replicas)
            .await?;

        Ok(())
    }

    /// Removes replica resources whose ordinal is greater than the desired replica count.
    ///
    /// # Arguments
    /// - `name` - Name of the PgOpr resource.
    /// - `namespace` - Namespace where the replica resources reside.
    /// - `desired_replicas` - Desired number of replicas.
    pub async fn cleanup_stale_replicas(
        &self,
        name: &str,
        namespace: &str,
        desired_replicas: u32,
    ) -> Result<(), Error> {
        let deploy_api: Api<Deployment> = Api::namespaced(self.manager.get_client(), namespace);
        for deployment in deploy_api.list(&ListParams::default()).await? {
            let resource_name = deployment.name_any();
            if replica_ordinal(name, &resource_name).is_some_and(|i| i > desired_replicas) {
                self.delete_replica_stack(&resource_name, namespace).await?;
            }
        }

        Ok(())
    }

    /// Deletes all resources that belong to the given cluster, including managed PVs.
    ///
    /// # Arguments
    /// - `pgopr` - The PgOpr resource that defines the cluster to clean.
    pub async fn cleanup_all(&self, pgopr: &pgopr) -> Result<(), Error> {
        let name = pgopr.name_any();
        let namespace = pgopr.namespace().unwrap_or_else(|| "default".to_string());

        let deploy_api: Api<Deployment> = Api::namespaced(self.manager.get_client(), &namespace);
        for deployment in deploy_api.list(&ListParams::default()).await? {
            let resource_name = deployment.name_any();
            if replica_ordinal(&name, &resource_name).is_some() {
                self.delete_replica_stack(&resource_name, &namespace)
                    .await?;
            }
        }

        self.manager.delete::<Service>(&name, &namespace).await?;
        self.manager.delete::<Deployment>(&name, &namespace).await?;
        self.manager
            .delete::<PersistentVolumeClaim>(&format!("{}-pv-claim", name), &namespace)
            .await?;

        // Cluster-scoped PV discovery and cleanup
        self.manager
            .delete_cluster_by_label::<PersistentVolume>(crate::manager::LABEL_CLUSTER, &name)
            .await?;

        Ok(())
    }

    async fn delete_replica_stack(&self, replica_name: &str, namespace: &str) -> Result<(), Error> {
        self.manager
            .delete::<Service>(replica_name, namespace)
            .await?;
        self.manager
            .delete::<Deployment>(replica_name, namespace)
            .await?;
        self.manager
            .delete::<PersistentVolumeClaim>(&format!("{}-pv-claim", replica_name), namespace)
            .await?;
        self.manager
            .delete_cluster::<PersistentVolume>(&format!("{}-pv-volume", replica_name))
            .await?;

        Ok(())
    }
}

fn replica_ordinal(cluster_name: &str, resource_name: &str) -> Option<u32> {
    let prefix = format!("{}-replica-", cluster_name);
    resource_name
        .strip_prefix(&prefix)
        .and_then(|suffix| suffix.split('-').next())
        .and_then(|ordinal| ordinal.parse::<u32>().ok())
}

fn condition(pgopr: &pgopr, type_: &str, status: &str, reason: &str, message: String) -> Condition {
    let last_transition_time = pgopr
        .status
        .as_ref()
        .and_then(|status| status.conditions.as_ref())
        .and_then(|conditions| {
            conditions
                .iter()
                .find(|condition| condition.type_ == type_)
                .and_then(|condition| {
                    if condition.status == status && condition.reason == reason {
                        Some(condition.last_transition_time.clone())
                    } else {
                        None
                    }
                })
        })
        .unwrap_or_else(|| Time(k8s_openapi::jiff::Timestamp::now()));

    Condition {
        type_: type_.to_string(),
        status: status.to_string(),
        observed_generation: None,
        last_transition_time,
        reason: reason.to_string(),
        message,
    }
}

fn is_failure_reason(reason: &str) -> bool {
    matches!(
        reason,
        "CrashLoopBackOff"
            | "ErrImagePull"
            | "ImagePullBackOff"
            | "CreateContainerConfigError"
            | "CreateContainerError"
            | "InvalidImageName"
            | "Error"
            | "OOMKilled"
    )
}
