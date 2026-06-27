/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

mod cleanup;
mod config;
mod status;
mod topology;

use crate::crd::v1::{PgMonetaSpec, pgopr};
use crate::manager::{self, ResourceManager};
use crate::workload::{DeploymentConfig, PG18_PRIMARY_IMAGE, PG18_REPLICA_IMAGE};
use crate::{Error, persistent, pgexporter, pgmoneta, primary, replica, services};
use config::ConfigResult;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{PersistentVolume, PersistentVolumeClaim, Secret};
use kube::{Api, Client};
use std::sync::Arc;
use topology::{ClusterMember, ClusterTopology};

/// Cluster represents the desired state of a PostgreSQL Star Configuration
pub struct Cluster {
    manager: ResourceManager,
}

impl Cluster {
    /// Creates a new Cluster manager instance.
    ///
    /// # Arguments
    /// - `client` - The Kubernetes client to use for resource operations.
    pub fn new(client: Client) -> Self {
        Self {
            manager: ResourceManager::new(client),
        }
    }

    /// Reconciles the desired state of the cluster and updates the PgOpr status.
    ///
    /// # Arguments
    /// - `pgopr` - The PgOpr resource defining the cluster state.
    pub async fn reconcile_state(&self, pgopr: Arc<pgopr>) -> Result<(), Error> {
        let topology = ClusterTopology::from_pgopr(&pgopr);

        let version = pgopr.spec.version.as_deref().unwrap_or("18");
        if version != "18" {
            let err = Error::UnsupportedPostgresVersion(version.to_string());
            let status = status::invalid_spec(&pgopr, err.to_string());
            self.patch_status(&topology, status).await?;
            return Ok(());
        }

        let config_info = if let Some(config) = &pgopr.spec.config {
            Some(config::sync_config(&self.manager, &pgopr, config).await?)
        } else {
            None
        };

        self.sync_topology(&pgopr, &topology, config_info).await?;

        let status = status::observe(&self.manager, &topology, &pgopr).await?;
        self.patch_status(&topology, status).await?;

        Ok(())
    }

    async fn patch_status(
        &self,
        topology: &ClusterTopology,
        status: crate::crd::v1::PgOprStatus,
    ) -> Result<(), Error> {
        let pgopr_api: Api<pgopr> =
            Api::namespaced(self.manager.get_client(), topology.namespace());
        let ps = kube::api::PatchParams::apply(manager::MANAGER_NAME);
        let api_version = format!("{}/{}", manager::API_GROUP, manager::VERSION_PGOPR);

        let _ = pgopr_api
            .patch_status(
                topology.name(),
                &ps,
                &kube::api::Patch::Apply(serde_json::json!({
                    "apiVersion": api_version,
                    "kind": manager::KIND_PGOPR,
                    "status": status
                })),
            )
            .await?;
        Ok(())
    }

    /// Removes replica resources that are no longer needed based on the desired replica count.
    ///
    /// # Arguments
    /// - `name` - The base name of the cluster.
    /// - `namespace` - The namespace where resources reside.
    /// - `desired_replicas` - The number of replicas requested in the spec.
    pub async fn cleanup_stale_replicas(
        &self,
        name: &str,
        namespace: &str,
        desired_replicas: u32,
    ) -> Result<(), Error> {
        cleanup::stale_replicas(&self.manager, name, namespace, desired_replicas).await
    }

    /// Deletes all Kubernetes resources belonging to the cluster.
    ///
    /// # Arguments
    /// - `pgopr` - The PgOpr resource being cleaned up.
    pub async fn cleanup_all(&self, pgopr: &pgopr) -> Result<(), Error> {
        let topology = ClusterTopology::from_pgopr(pgopr);
        cleanup::all(&self.manager, &topology).await
    }

    async fn sync_topology(
        &self,
        pgopr: &Arc<pgopr>,
        topology: &ClusterTopology,
        config_info: Option<ConfigResult>,
    ) -> Result<(), Error> {
        let primary = topology.primary();

        let primary_config = DeploymentConfig {
            image: PG18_PRIMARY_IMAGE,
            resources: pgopr.spec.resources.as_ref(),
            config_map_name: config_info.as_ref().map(|c| c.name.as_str()),
            config_hash: config_info.as_ref().map(|c| c.hash.as_str()),
        };
        self.sync_primary(pgopr, topology, &primary, primary_config)
            .await?;

        for member in topology.replica_members() {
            let replica_config = DeploymentConfig {
                image: PG18_REPLICA_IMAGE,
                resources: pgopr.spec.resources.as_ref(),
                config_map_name: config_info.as_ref().map(|c| c.name.as_str()),
                config_hash: config_info.as_ref().map(|c| c.hash.as_str()),
            };
            self.sync_replica(pgopr, topology, &member, replica_config)
                .await?;
        }

        self.cleanup_stale_replicas(topology.name(), topology.namespace(), topology.replicas())
            .await?;

        if let Some(pgmoneta_spec) = &pgopr.spec.pgmoneta {
            self.sync_pgmoneta(pgopr, topology, pgmoneta_spec).await?;
        } else {
            self.cleanup_pgmoneta(topology).await?;
        }

        if pgopr.spec.pgexporter.is_some() {
            self.sync_pgexporter(pgopr, topology).await?
        } else {
            self.cleanup_pgexporter(topology).await?
        }

        Ok(())
    }

    async fn sync_primary(
        &self,
        pgopr: &Arc<pgopr>,
        topology: &ClusterTopology,
        member: &ClusterMember,
        config: DeploymentConfig<'_>,
    ) -> Result<(), Error> {
        self.sync_storage(pgopr, topology, member).await?;

        let deployment = primary::build(member.name(), topology.namespace(), config);
        self.manager.sync(pgopr, deployment).await?;

        let service = services::build(member.name(), topology.namespace(), 5432);
        self.manager.sync(pgopr, service).await?;

        Ok(())
    }

    async fn sync_replica(
        &self,
        pgopr: &Arc<pgopr>,
        topology: &ClusterTopology,
        member: &ClusterMember,
        config: DeploymentConfig<'_>,
    ) -> Result<(), Error> {
        self.sync_storage(pgopr, topology, member).await?;

        let slot_name = member.slot_name().ok_or_else(|| {
            Error::UserInputError(format!("Replica {} is missing a slot name", member.name()))
        })?;
        let deployment = replica::build(
            member.name(),
            topology.name(),
            topology.namespace(),
            slot_name,
            config,
        );
        self.manager.sync(pgopr, deployment).await?;

        let service = services::build(member.name(), topology.namespace(), 5432);
        self.manager.sync(pgopr, service).await?;

        Ok(())
    }

    async fn sync_storage(
        &self,
        pgopr: &Arc<pgopr>,
        topology: &ClusterTopology,
        member: &ClusterMember,
    ) -> Result<(), Error> {
        let pv = persistent::build_pv(
            &member.pv_name(),
            topology.storage(),
            member.name(),
            member.host_path(),
            topology.name(),
        );
        self.manager.sync_cluster(pv).await?;

        let pvc = persistent::build_pvc(
            &member.pvc_name(),
            topology.namespace(),
            topology.storage(),
            member.name(),
        );
        self.manager.sync(pgopr, pvc).await?;

        Ok(())
    }

    async fn sync_pgmoneta(
        &self,
        pgopr: &Arc<pgopr>,
        topology: &ClusterTopology,
        spec: &PgMonetaSpec,
    ) -> Result<(), Error> {
        let storage = spec.storage.unwrap_or(10);
        let host_path = format!("/tmp/kind-pgmoneta-{}", topology.name());

        let pv = persistent::build_pgmoneta_pv(
            &topology.pgmoneta_pv_name(),
            storage,
            &host_path,
            topology.name(),
        );
        self.manager.sync_cluster(pv).await?;

        let pvc = persistent::build_pvc(
            &topology.pgmoneta_pvc_name(),
            topology.namespace(),
            storage,
            &topology.pgmoneta_name(),
        );
        self.manager.sync(pgopr, pvc).await?;

        let secret = pgmoneta::build_secret(
            &topology.pgmoneta_secret_name(),
            topology.namespace(),
            "backup_pass",
        );
        self.manager.sync(pgopr, secret).await?;

        let deployment = pgmoneta::build_deployment(
            &topology.pgmoneta_name(),
            topology.namespace(),
            topology.name(),
            &topology.pgmoneta_pvc_name(),
            &topology.pgmoneta_secret_name(),
        );
        self.manager.sync(pgopr, deployment).await?;

        Ok(())
    }

    async fn cleanup_pgmoneta(&self, topology: &ClusterTopology) -> Result<(), Error> {
        self.manager
            .delete::<Deployment>(&topology.pgmoneta_name(), topology.namespace())
            .await?;
        self.manager
            .delete::<PersistentVolumeClaim>(&topology.pgmoneta_pvc_name(), topology.namespace())
            .await?;
        self.manager
            .delete::<Secret>(&topology.pgmoneta_secret_name(), topology.namespace())
            .await?;
        self.manager
            .delete_cluster::<PersistentVolume>(&topology.pgmoneta_pv_name())
            .await?;

        Ok(())
    }
    async fn sync_pgexporter(
        &self,
        pgopr: &Arc<pgopr>,
        topology: &ClusterTopology,
    ) -> Result<(), Error> {
        let secret = pgexporter::build_secret(
            &topology.pgexporter_secret_name(),
            topology.namespace(),
            "pgexporter_pass",
        );
        self.manager.sync(pgopr, secret).await?;

        let deployment = pgexporter::build_deployment(
            &topology.pgexporter_name(),
            topology.namespace(),
            topology.name(),
            &topology.pgexporter_secret_name(),
            pgopr
                .spec
                .pgexporter
                .as_ref()
                .and_then(|s| s.resources.as_ref()),
        );
        self.manager.sync(pgopr, deployment).await?;
        let svc = services::build(&topology.pgexporter_name(), topology.namespace(), 5002);
        self.manager.sync(pgopr, svc).await?;

        if pgopr
            .spec
            .pgexporter
            .as_ref()
            .and_then(|s| s.monitoring.as_ref())
            .is_some()
        {
            self.sync_pgexporter_monitoring(pgopr, topology).await?
        } else {
            self.cleanup_pgexporter_monitoring(topology).await?
        }

        Ok(())
    }

    async fn cleanup_pgexporter(&self, topology: &ClusterTopology) -> Result<(), Error> {
        self.manager
            .delete::<Deployment>(&topology.pgexporter_name(), topology.namespace())
            .await?;
        self.manager
            .delete::<Secret>(&topology.pgexporter_secret_name(), topology.namespace())
            .await?;

        Ok(())
    }
    async fn sync_pgexporter_monitoring(
        &self,
        pgopr: &Arc<pgopr>,
        topology: &ClusterTopology,
    ) -> Result<(), Error> {
        let deployment = pgexporter::build_monitoring_deployment(
            &topology.pgexporter_mon_name(),
            topology.namespace(),
            &topology.pgexporter_name(),
            pgopr
                .spec
                .pgexporter
                .as_ref()
                .and_then(|s| s.monitoring.as_ref())
                .and_then(|m| m.resources.as_ref()),
        );
        self.manager.sync(pgopr, deployment).await?;
        Ok(())
    }
    async fn cleanup_pgexporter_monitoring(&self, topology: &ClusterTopology) -> Result<(), Error> {
        self.manager
            .delete::<Deployment>(&topology.pgexporter_mon_name(), topology.namespace())
            .await?;
        Ok(())
    }
}
