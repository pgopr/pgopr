/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

mod cleanup;
mod status;
mod topology;

use crate::crd::v1::pgopr;
use crate::manager::{self, ResourceManager};
use crate::{Error, persistent, primary, replica, services};
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

        self.sync_topology(&pgopr, &topology).await?;

        let status = status::observe(&self.manager, &topology, &pgopr).await?;
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
    ) -> Result<(), Error> {
        let primary = topology.primary();
        self.sync_primary(pgopr, topology, &primary).await?;

        for member in topology.replica_members() {
            self.sync_replica(pgopr, topology, &member).await?;
        }

        self.cleanup_stale_replicas(topology.name(), topology.namespace(), topology.replicas())
            .await?;

        Ok(())
    }

    async fn sync_primary(
        &self,
        pgopr: &Arc<pgopr>,
        topology: &ClusterTopology,
        member: &ClusterMember,
    ) -> Result<(), Error> {
        self.sync_storage(pgopr, topology, member).await?;

        let deployment = primary::build(member.name(), topology.namespace());
        self.manager.sync(pgopr, deployment).await?;

        let service = services::build(member.name(), topology.namespace());
        self.manager.sync(pgopr, service).await?;

        Ok(())
    }

    async fn sync_replica(
        &self,
        pgopr: &Arc<pgopr>,
        topology: &ClusterTopology,
        member: &ClusterMember,
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
        );
        self.manager.sync(pgopr, deployment).await?;

        let service = services::build(member.name(), topology.namespace());
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
}
