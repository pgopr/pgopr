/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use super::topology::ClusterTopology;
use crate::Error;
use crate::crd::v1::PgExporterStatus;
use crate::crd::v1::{
    DeploymentStatus, PgMonetaStatus, PgOprStatus, ServiceStatus, StorageStatus, pgopr,
};
use crate::manager::ResourceManager;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{PersistentVolume, PersistentVolumeClaim, Pod, Service};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::Resource;
use kube::{Api, ResourceExt, api::ListParams};
use std::collections::BTreeMap;

// Kubernetes resource phase values
const PHASE_BOUND: &str = "Bound";
const PHASE_PENDING: &str = "Pending";
const PHASE_RUNNING: &str = "Running";
const PHASE_DEGRADED: &str = "Degraded";
const PHASE_FAILED: &str = "Failed";

// Kubernetes condition type
const CONDITION_READY: &str = "Ready";
const CONDITION_STATUS_TRUE: &str = "True";
const CONDITION_STATUS_FALSE: &str = "False";

// Kubernetes reason
const REASON_POD_FAILURE: &str = "PodFailure";
const REASON_CLUSTER_READY: &str = "ClusterReady";
const REASON_REPLICAS_NOT_READY: &str = "ReplicasNotReady";
const REASON_PRIMARY_NOT_READY: &str = "PrimaryNotReady";
const REASON_INVALID_SPEC: &str = "InvalidSpec";

/// Builds status for a PgOpr resource whose spec cannot be reconciled.
///
/// # Arguments
/// - `pgopr` - The PgOpr resource defining the invalid desired state.
/// - `message` - The reason the spec cannot be reconciled.
pub(super) fn invalid_spec(pgopr: &pgopr, message: String) -> PgOprStatus {
    PgOprStatus {
        phase: PHASE_FAILED.to_string(),
        conditions: Some(vec![condition(
            pgopr,
            CONDITION_READY,
            CONDITION_STATUS_FALSE,
            REASON_INVALID_SPEC,
            message,
        )]),
        ..Default::default()
    }
}

/// Observes the current state of all Kubernetes resources belonging to this cluster.
///
/// # Arguments
/// - `manager` - The Kubernetes resource manager.
/// - `topology` - The expected cluster topology.
/// - `pgopr` - The PgOpr resource defining the cluster identity and desired state.
pub(super) async fn observe(
    manager: &ResourceManager,
    topology: &ClusterTopology,
    pgopr: &pgopr,
) -> Result<PgOprStatus, Error> {
    let mut status = PgOprStatus {
        phase: PHASE_PENDING.to_string(),
        ..Default::default()
    };

    observe_deployments(manager, topology, &mut status).await?;
    observe_services(manager, topology, &mut status).await?;
    observe_storage(manager, topology, &mut status).await?;
    observe_pgmoneta(manager, topology, pgopr, &mut status).await?;
    observe_pgexporter(manager, topology, pgopr, &mut status).await?;
    finalize(pgopr, topology, &mut status);

    Ok(status)
}

async fn observe_deployments(
    manager: &ResourceManager,
    topology: &ClusterTopology,
    status: &mut PgOprStatus,
) -> Result<(), Error> {
    let deploy_api: Api<Deployment> = Api::namespaced(manager.get_client(), topology.namespace());

    if let Ok(primary) = deploy_api.get(topology.name()).await {
        let reason = pod_failure_reason(manager, topology.namespace(), topology.name()).await?;
        status.primary = Some(deployment_status(topology.name(), &primary, reason));
    }

    for member in topology.replica_members() {
        if let Ok(replica) = deploy_api.get(member.name()).await {
            let reason = pod_failure_reason(manager, topology.namespace(), member.name()).await?;
            status
                .replicas
                .push(deployment_status(member.name(), &replica, reason));
        }
    }

    Ok(())
}

async fn observe_services(
    manager: &ResourceManager,
    topology: &ClusterTopology,
    status: &mut PgOprStatus,
) -> Result<(), Error> {
    let svc_api: Api<Service> = Api::namespaced(manager.get_client(), topology.namespace());
    let services = svc_api.list(&ListParams::default()).await?;
    let services_by_name: BTreeMap<String, Service> = services
        .into_iter()
        .map(|service| (service.name_any(), service))
        .collect();

    for name in topology.member_names() {
        if let Some(service) = services_by_name.get(&name) {
            status.services.push(service_status(service));
        }
    }

    Ok(())
}

async fn observe_storage(
    manager: &ResourceManager,
    topology: &ClusterTopology,
    status: &mut PgOprStatus,
) -> Result<(), Error> {
    let pvc_api: Api<PersistentVolumeClaim> =
        Api::namespaced(manager.get_client(), topology.namespace());
    let pv_api: Api<PersistentVolume> = Api::all(manager.get_client());

    let pvcs = pvc_api.list(&ListParams::default()).await?;
    let pvcs_by_name: BTreeMap<String, PersistentVolumeClaim> =
        pvcs.into_iter().map(|pvc| (pvc.name_any(), pvc)).collect();

    for name in topology.pvc_names() {
        if let Some(pvc) = pvcs_by_name.get(&name) {
            status.storage.push(pvc_status(pvc));
        }
    }

    let pvs = pv_api
        .list(&ListParams::default().labels(&topology.pv_selector()))
        .await?;
    for pv in pvs {
        status.storage.push(pv_status(pv));
    }

    Ok(())
}

async fn observe_pgmoneta(
    manager: &ResourceManager,
    topology: &ClusterTopology,
    pgopr: &pgopr,
    status: &mut PgOprStatus,
) -> Result<(), Error> {
    if pgopr.spec.pgmoneta.is_none() {
        return Ok(());
    }

    let deploy_api: Api<Deployment> = Api::namespaced(manager.get_client(), topology.namespace());
    let pvc_api: Api<PersistentVolumeClaim> =
        Api::namespaced(manager.get_client(), topology.namespace());

    let deployment_exists = deploy_api.get(&topology.pgmoneta_name()).await.ok();
    let pvc = pvc_api.get(&topology.pgmoneta_pvc_name()).await.ok();

    let pod_reason = if let Some(ref _d) = deployment_exists {
        pod_failure_reason(manager, topology.namespace(), &topology.pgmoneta_name()).await?
    } else {
        None
    };

    let deploy_status = deployment_exists
        .as_ref()
        .map(|d| deployment_status(&topology.pgmoneta_name(), d, pod_reason));

    let storage_status = pvc.as_ref().map(pvc_status);

    let ready = deploy_status.as_ref().is_some_and(|d| d.available)
        && storage_status.as_ref().is_some_and(|s| s.bound);

    let (reason, message) = if ready {
        (None, None)
    } else if deployment_exists.is_none() {
        (
            Some("DeploymentNotFound".to_string()),
            Some("pgmoneta Deployment does not exist".to_string()),
        )
    } else if !deploy_status.as_ref().is_some_and(|d| d.available) {
        (
            Some("DeploymentNotReady".to_string()),
            Some("pgmoneta Deployment exists but is not ready".to_string()),
        )
    } else if pvc.is_none() {
        (
            Some("StorageNotFound".to_string()),
            Some("pgmoneta PVC does not exist".to_string()),
        )
    } else {
        (
            Some("StorageNotBound".to_string()),
            Some("pgmoneta PVC exists but is not bound".to_string()),
        )
    };

    status.pgmoneta = Some(PgMonetaStatus {
        deployment: deploy_status,
        storage: storage_status,
        ready,
        reason,
        message,
    });

    Ok(())
}
async fn observe_pgexporter(
    manager: &ResourceManager,
    topology: &ClusterTopology,
    pgopr: &pgopr,
    status: &mut PgOprStatus,
) -> Result<(), Error> {
    if pgopr.spec.pgexporter.is_none() {
        return Ok(());
    }

    let deploy_api: Api<Deployment> = Api::namespaced(manager.get_client(), topology.namespace());

    let deployment_exists = deploy_api.get(&topology.pgexporter_name()).await.ok();

    let pod_reason = if let Some(ref _d) = deployment_exists {
        pod_failure_reason(manager, topology.namespace(), &topology.pgexporter_name()).await?
    } else {
        None
    };

    let deploy_status = deployment_exists
        .as_ref()
        .map(|d| deployment_status(&topology.pgexporter_name(), d, pod_reason));

    let monitoring_exists = deploy_api.get(&topology.pgexporter_mon_name()).await.ok();

    let mon_pod_reason = if let Some(ref _d) = monitoring_exists {
        pod_failure_reason(
            manager,
            topology.namespace(),
            &topology.pgexporter_mon_name(),
        )
        .await?
    } else {
        None
    };

    let mon_status = monitoring_exists
        .as_ref()
        .map(|d| deployment_status(&topology.pgexporter_mon_name(), d, mon_pod_reason));

    let ready = deploy_status.as_ref().is_some_and(|d| d.available);
    let (reason, message) = if ready {
        (None, None)
    } else if deployment_exists.is_none() {
        (
            Some("DeploymentNotFound".to_string()),
            Some("pgexporter Deployment does not exist".to_string()),
        )
    } else {
        (
            Some("DeploymentNotReady".to_string()),
            Some("pgexporter Deployment exists but is not ready".to_string()),
        )
    };

    status.pgexporter = Some(PgExporterStatus {
        deployment: deploy_status,
        monitoring: mon_status,
        ready,
        reason,
        message,
    });

    Ok(())
}
fn finalize(pgopr: &pgopr, topology: &ClusterTopology, status: &mut PgOprStatus) {
    let primary_ready = status.primary.as_ref().is_some_and(|p| p.available);
    let replicas_ready = status.replicas.iter().filter(|r| r.available).count() as u32;

    if let Some(reason) = failure_reason(status) {
        status.phase = PHASE_FAILED.to_string();
        status.conditions = Some(vec![condition(
            pgopr,
            CONDITION_READY,
            CONDITION_STATUS_FALSE,
            REASON_POD_FAILURE,
            format!("A PostgreSQL pod is not ready: {}", reason),
        )]);
    } else if primary_ready && replicas_ready == topology.replicas() {
        status.phase = PHASE_RUNNING.to_string();
        status.conditions = Some(vec![condition(
            pgopr,
            CONDITION_READY,
            CONDITION_STATUS_TRUE,
            REASON_CLUSTER_READY,
            "All PostgreSQL deployments are ready".to_string(),
        )]);
    } else if primary_ready {
        status.phase = PHASE_DEGRADED.to_string();
        status.conditions = Some(vec![condition(
            pgopr,
            CONDITION_READY,
            CONDITION_STATUS_FALSE,
            REASON_REPLICAS_NOT_READY,
            "Primary is ready but one or more replicas are not ready".to_string(),
        )]);
    } else {
        status.conditions = Some(vec![condition(
            pgopr,
            CONDITION_READY,
            CONDITION_STATUS_FALSE,
            REASON_PRIMARY_NOT_READY,
            "Primary deployment is not ready".to_string(),
        )]);
    }
}

async fn pod_failure_reason(
    manager: &ResourceManager,
    namespace: &str,
    app: &str,
) -> Result<Option<String>, Error> {
    let pod_api: Api<Pod> = Api::namespaced(manager.get_client(), namespace);
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

fn deployment_status(
    name: &str,
    deployment: &Deployment,
    reason: Option<String>,
) -> DeploymentStatus {
    let ready_replicas = deployment
        .status
        .as_ref()
        .and_then(|s| s.ready_replicas)
        .unwrap_or(0) as u32;

    DeploymentStatus {
        name: name.to_string(),
        ready_replicas,
        desired_replicas: 1,
        available: ready_replicas > 0,
        reason,
    }
}

fn service_status(service: &Service) -> ServiceStatus {
    ServiceStatus {
        name: service.name_any(),
        type_: service.spec.as_ref().and_then(|s| s.type_.clone()),
        cluster_ip: service.spec.as_ref().and_then(|s| s.cluster_ip.clone()),
        ready: service.spec.is_some(),
    }
}

fn pvc_status(pvc: &PersistentVolumeClaim) -> StorageStatus {
    StorageStatus {
        name: pvc.name_any(),
        kind: PersistentVolumeClaim::kind(&()).to_string(),
        bound: pvc
            .status
            .as_ref()
            .is_some_and(|s| s.phase.as_ref().is_some_and(|p| p == PHASE_BOUND)),
    }
}

fn pv_status(pv: PersistentVolume) -> StorageStatus {
    StorageStatus {
        name: pv.name_any(),
        kind: PersistentVolume::kind(&()).to_string(),
        bound: pv
            .status
            .is_some_and(|s| s.phase.is_some_and(|p| p == PHASE_BOUND)),
    }
}

fn failure_reason(status: &PgOprStatus) -> Option<String> {
    status
        .primary
        .as_ref()
        .and_then(|primary| primary.reason.clone())
        .or_else(|| {
            status
                .replicas
                .iter()
                .find_map(|replica| replica.reason.clone())
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crd::v1::PgOprSpec;
    use k8s_openapi::api::apps::v1::DeploymentStatus as K8sDeploymentStatus;
    use k8s_openapi::api::core::v1::{
        PersistentVolumeClaimStatus, PersistentVolumeStatus, ServiceSpec,
    };
    use kube::api::ObjectMeta;

    fn resource_with_replicas(replicas: u32, status: Option<PgOprStatus>) -> pgopr {
        pgopr {
            metadata: ObjectMeta {
                name: Some("postgresql".to_string()),
                namespace: Some("default".to_string()),
                ..Default::default()
            },
            spec: PgOprSpec {
                version: None,
                storage: 5,
                replicas: Some(replicas),
                resources: None,
                config: None,
                pgmoneta: None,
                pgexporter: None,
            },
            status,
        }
    }

    fn resource(status: Option<PgOprStatus>) -> pgopr {
        resource_with_replicas(2, status)
    }

    fn topology(replicas: u32) -> ClusterTopology {
        ClusterTopology::from_pgopr(&resource_with_replicas(replicas, None))
    }

    fn deployment(name: &str, ready_replicas: Option<i32>) -> Deployment {
        Deployment {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                ..Default::default()
            },
            status: Some(K8sDeploymentStatus {
                ready_replicas,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn member_status(name: &str, available: bool, reason: Option<&str>) -> DeploymentStatus {
        DeploymentStatus {
            name: name.to_string(),
            ready_replicas: u32::from(available),
            desired_replicas: 1,
            available,
            reason: reason.map(str::to_string),
        }
    }

    fn ready_status() -> PgOprStatus {
        PgOprStatus {
            primary: Some(member_status("postgresql", true, None)),
            replicas: vec![
                member_status("postgresql-replica-1", true, None),
                member_status("postgresql-replica-2", true, None),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn test_invalid_spec_sets_failed_phase_and_condition() {
        let status = invalid_spec(&resource(None), "storage must be positive".to_string());

        assert_eq!(status.phase, PHASE_FAILED);
        let conditions = status.conditions.expect("conditions");
        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].type_, CONDITION_READY);
        assert_eq!(conditions[0].status, CONDITION_STATUS_FALSE);
        assert_eq!(conditions[0].reason, REASON_INVALID_SPEC);
        assert_eq!(conditions[0].message, "storage must be positive");
    }

    #[test]
    fn test_finalize_running_when_primary_and_all_replicas_are_ready() {
        let mut status = ready_status();

        finalize(&resource(None), &topology(2), &mut status);

        assert_eq!(status.phase, PHASE_RUNNING);
        let conditions = status.conditions.expect("conditions");
        assert_eq!(conditions[0].status, CONDITION_STATUS_TRUE);
        assert_eq!(conditions[0].reason, REASON_CLUSTER_READY);
    }

    #[test]
    fn test_finalize_degraded_when_a_replica_is_not_ready() {
        let mut status = ready_status();
        status.replicas[1] = member_status("postgresql-replica-2", false, None);

        finalize(&resource(None), &topology(2), &mut status);

        assert_eq!(status.phase, PHASE_DEGRADED);
        let conditions = status.conditions.expect("conditions");
        assert_eq!(conditions[0].status, CONDITION_STATUS_FALSE);
        assert_eq!(conditions[0].reason, REASON_REPLICAS_NOT_READY);
    }

    #[test]
    fn test_finalize_stays_pending_when_primary_is_not_ready() {
        let mut status = PgOprStatus {
            phase: PHASE_PENDING.to_string(),
            primary: Some(member_status("postgresql", false, None)),
            ..Default::default()
        };

        finalize(&resource(None), &topology(0), &mut status);

        assert_eq!(status.phase, PHASE_PENDING);
        let conditions = status.conditions.expect("conditions");
        assert_eq!(conditions[0].status, CONDITION_STATUS_FALSE);
        assert_eq!(conditions[0].reason, REASON_PRIMARY_NOT_READY);
    }

    #[test]
    fn test_finalize_failed_when_primary_reports_a_failure_reason() {
        let mut status = ready_status();
        status.primary = Some(member_status("postgresql", false, Some("CrashLoopBackOff")));

        finalize(&resource(None), &topology(2), &mut status);

        assert_eq!(status.phase, PHASE_FAILED);
        let conditions = status.conditions.expect("conditions");
        assert_eq!(conditions[0].reason, REASON_POD_FAILURE);
        assert!(conditions[0].message.contains("CrashLoopBackOff"));
    }

    #[test]
    fn test_finalize_failed_when_replica_reports_a_failure_reason() {
        let mut status = ready_status();
        status.replicas[0] = member_status("postgresql-replica-1", false, Some("OOMKilled"));

        finalize(&resource(None), &topology(2), &mut status);

        assert_eq!(status.phase, PHASE_FAILED);
        let conditions = status.conditions.expect("conditions");
        assert_eq!(conditions[0].reason, REASON_POD_FAILURE);
        assert!(conditions[0].message.contains("OOMKilled"));
    }

    #[test]
    fn test_failure_reason_prefers_primary_and_falls_back_to_replicas() {
        let mut status = ready_status();
        status.primary = Some(member_status("postgresql", false, Some("PrimaryError")));
        status.replicas[0] = member_status("postgresql-replica-1", false, Some("ReplicaError"));

        assert_eq!(failure_reason(&status).as_deref(), Some("PrimaryError"));

        status.primary = Some(member_status("postgresql", true, None));

        assert_eq!(failure_reason(&status).as_deref(), Some("ReplicaError"));

        status.replicas[0] = member_status("postgresql-replica-1", true, None);

        assert_eq!(failure_reason(&status), None);
    }

    #[test]
    fn test_deployment_status_uses_ready_replicas() {
        let status = deployment_status("postgresql", &deployment("postgresql", Some(2)), None);

        assert_eq!(status.name, "postgresql");
        assert_eq!(status.ready_replicas, 2);
        assert_eq!(status.desired_replicas, 1);
        assert!(status.available);
        assert_eq!(status.reason, None);
    }

    #[test]
    fn test_deployment_status_is_unavailable_without_ready_replicas() {
        let missing = deployment_status("postgresql", &deployment("postgresql", None), None);
        assert_eq!(missing.ready_replicas, 0);
        assert!(!missing.available);

        let zero = deployment_status(
            "postgresql",
            &deployment("postgresql", Some(0)),
            Some("ImagePullBackOff".to_string()),
        );
        assert_eq!(zero.ready_replicas, 0);
        assert!(!zero.available);
        assert_eq!(zero.reason.as_deref(), Some("ImagePullBackOff"));
    }

    #[test]
    fn test_service_status_maps_type_and_cluster_ip() {
        let service = Service {
            metadata: ObjectMeta {
                name: Some("postgresql".to_string()),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                type_: Some("ClusterIP".to_string()),
                cluster_ip: Some("10.0.0.10".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let status = service_status(&service);

        assert_eq!(status.name, "postgresql");
        assert_eq!(status.type_.as_deref(), Some("ClusterIP"));
        assert_eq!(status.cluster_ip.as_deref(), Some("10.0.0.10"));
        assert!(status.ready);
    }

    #[test]
    fn test_service_status_without_spec_is_not_ready() {
        let service = Service {
            metadata: ObjectMeta {
                name: Some("postgresql".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let status = service_status(&service);

        assert_eq!(status.name, "postgresql");
        assert_eq!(status.type_, None);
        assert_eq!(status.cluster_ip, None);
        assert!(!status.ready);
    }

    #[test]
    fn test_pvc_status_requires_bound_phase() {
        let bound = PersistentVolumeClaim {
            metadata: ObjectMeta {
                name: Some("postgresql-pv-claim".to_string()),
                ..Default::default()
            },
            status: Some(PersistentVolumeClaimStatus {
                phase: Some(PHASE_BOUND.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let pending = PersistentVolumeClaim {
            metadata: ObjectMeta {
                name: Some("postgresql-pv-claim".to_string()),
                ..Default::default()
            },
            status: Some(PersistentVolumeClaimStatus {
                phase: Some(PHASE_PENDING.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let unknown = PersistentVolumeClaim {
            metadata: ObjectMeta {
                name: Some("postgresql-pv-claim".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(pvc_status(&bound).bound);
        assert!(!pvc_status(&pending).bound);
        assert!(!pvc_status(&unknown).bound);
    }

    #[test]
    fn test_pv_status_requires_bound_phase() {
        let bound = PersistentVolume {
            metadata: ObjectMeta {
                name: Some("postgresql-pv-volume".to_string()),
                ..Default::default()
            },
            status: Some(PersistentVolumeStatus {
                phase: Some(PHASE_BOUND.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let available = PersistentVolume {
            metadata: ObjectMeta {
                name: Some("postgresql-pv-volume".to_string()),
                ..Default::default()
            },
            status: Some(PersistentVolumeStatus {
                phase: Some("Available".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert!(pv_status(bound).bound);
        assert!(!pv_status(available).bound);
    }

    #[test]
    fn test_is_failure_reason_recognizes_pod_failures() {
        for reason in [
            "CrashLoopBackOff",
            "ErrImagePull",
            "ImagePullBackOff",
            "CreateContainerConfigError",
            "CreateContainerError",
            "InvalidImageName",
            "Error",
            "OOMKilled",
        ] {
            assert!(is_failure_reason(reason), "{reason} should be a failure");
        }

        assert!(!is_failure_reason("Running"));
        assert!(!is_failure_reason(""));
    }

    #[test]
    fn test_condition_preserves_transition_time_for_same_state() {
        let timestamp = Time(k8s_openapi::jiff::Timestamp::from_second(1_700_000_000).unwrap());
        let existing = Condition {
            type_: CONDITION_READY.to_string(),
            status: CONDITION_STATUS_TRUE.to_string(),
            observed_generation: None,
            last_transition_time: timestamp.clone(),
            reason: REASON_CLUSTER_READY.to_string(),
            message: "old message".to_string(),
        };
        let resource = resource(Some(PgOprStatus {
            conditions: Some(vec![existing]),
            ..Default::default()
        }));

        let updated = condition(
            &resource,
            CONDITION_READY,
            CONDITION_STATUS_TRUE,
            REASON_CLUSTER_READY,
            "new message".to_string(),
        );

        assert_eq!(updated.last_transition_time, timestamp);
        assert_eq!(updated.message, "new message");
    }

    #[test]
    fn test_condition_uses_new_transition_time_when_state_changes() {
        let old_timestamp = Time(k8s_openapi::jiff::Timestamp::from_second(1_700_000_000).unwrap());
        let existing = Condition {
            type_: CONDITION_READY.to_string(),
            status: CONDITION_STATUS_TRUE.to_string(),
            observed_generation: None,
            last_transition_time: old_timestamp.clone(),
            reason: REASON_CLUSTER_READY.to_string(),
            message: "old message".to_string(),
        };
        let resource = resource(Some(PgOprStatus {
            conditions: Some(vec![existing]),
            ..Default::default()
        }));

        let updated = condition(
            &resource,
            CONDITION_READY,
            CONDITION_STATUS_FALSE,
            REASON_PRIMARY_NOT_READY,
            "primary is not ready".to_string(),
        );

        assert_ne!(updated.last_transition_time, old_timestamp);
    }
}
