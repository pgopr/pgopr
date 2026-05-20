/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use super::topology::{self, ClusterTopology};
use crate::Error;
use crate::manager::{self, ResourceManager};
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{PersistentVolume, PersistentVolumeClaim, Service};
use kube::{Api, ResourceExt, api::ListParams};

/// Removes replica resources that are no longer needed based on the desired replica count.
///
/// # Arguments
/// - `manager` - The Kubernetes resource manager.
/// - `name` - The base name of the cluster.
/// - `namespace` - The namespace where resources reside.
/// - `desired_replicas` - The number of replicas requested in the spec.
pub(super) async fn stale_replicas(
    manager: &ResourceManager,
    name: &str,
    namespace: &str,
    desired_replicas: u32,
) -> Result<(), Error> {
    let deploy_api: Api<Deployment> = Api::namespaced(manager.get_client(), namespace);
    for deployment in deploy_api.list(&ListParams::default()).await? {
        let resource_name = deployment.name_any();
        if topology::replica_ordinal(name, &resource_name).is_some_and(|i| i > desired_replicas) {
            delete_replica_stack(manager, &resource_name, namespace).await?;
        }
    }

    Ok(())
}

/// Deletes all Kubernetes resources belonging to the cluster.
///
/// # Arguments
/// - `manager` - The Kubernetes resource manager.
/// - `topology` - The expected cluster topology.
pub(super) async fn all(
    manager: &ResourceManager,
    topology: &ClusterTopology,
) -> Result<(), Error> {
    let deploy_api: Api<Deployment> = Api::namespaced(manager.get_client(), topology.namespace());
    for deployment in deploy_api.list(&ListParams::default()).await? {
        let resource_name = deployment.name_any();
        if topology::replica_ordinal(topology.name(), &resource_name).is_some() {
            delete_replica_stack(manager, &resource_name, topology.namespace()).await?;
        }
    }

    let primary = topology.primary();
    manager
        .delete::<Service>(primary.name(), topology.namespace())
        .await?;
    manager
        .delete::<Deployment>(primary.name(), topology.namespace())
        .await?;
    manager
        .delete::<PersistentVolumeClaim>(&primary.pvc_name(), topology.namespace())
        .await?;

    // Cluster-scoped PV discovery and cleanup
    manager
        .delete_cluster_by_label::<PersistentVolume>(manager::LABEL_CLUSTER, topology.name())
        .await?;

    Ok(())
}

async fn delete_replica_stack(
    manager: &ResourceManager,
    replica_name: &str,
    namespace: &str,
) -> Result<(), Error> {
    manager.delete::<Service>(replica_name, namespace).await?;
    manager
        .delete::<Deployment>(replica_name, namespace)
        .await?;
    manager
        .delete::<PersistentVolumeClaim>(&topology::pvc_name(replica_name), namespace)
        .await?;
    manager
        .delete_cluster::<PersistentVolume>(&topology::pv_name(replica_name))
        .await?;

    Ok(())
}
