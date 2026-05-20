/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use crate::crd::v1::pgopr;
use crate::manager;
use kube::ResourceExt;
use std::collections::BTreeSet;

const PRIMARY_HOST_PATH: &str = "/tmp/kind";
const REPLICA_HOST_PATH_PREFIX: &str = "/tmp/kind-replica-";
const REPLICA_NAME_SEGMENT: &str = "replica";
const PV_NAME_SUFFIX: &str = "pv-volume";
const PVC_NAME_SUFFIX: &str = "pv-claim";

/// ClusterTopology centralizes names and desired members for a PostgreSQL cluster.
pub(super) struct ClusterTopology {
    name: String,
    namespace: String,
    storage: u32,
    replicas: u32,
}

impl ClusterTopology {
    /// Builds topology data from the PgOpr resource.
    ///
    /// # Arguments
    /// - `pgopr` - The PgOpr resource defining the desired cluster state.
    pub(super) fn from_pgopr(pgopr: &pgopr) -> Self {
        Self {
            name: pgopr.name_any(),
            namespace: pgopr
                .namespace()
                .unwrap_or_else(|| manager::DEFAULT_NAMESPACE.to_string()),
            storage: pgopr.spec.storage,
            replicas: pgopr.spec.replicas.unwrap_or(0),
        }
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn namespace(&self) -> &str {
        &self.namespace
    }

    pub(super) fn storage(&self) -> u32 {
        self.storage
    }

    pub(super) fn replicas(&self) -> u32 {
        self.replicas
    }

    pub(super) fn primary(&self) -> ClusterMember {
        ClusterMember::primary(self.name.clone())
    }

    pub(super) fn replica_members(&self) -> Vec<ClusterMember> {
        (1..=self.replicas)
            .map(|ordinal| ClusterMember::replica(&self.name, ordinal))
            .collect()
    }

    pub(super) fn member_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        names.insert(self.name.clone());
        for member in self.replica_members() {
            names.insert(member.name().to_string());
        }
        names
    }

    pub(super) fn pvc_names(&self) -> BTreeSet<String> {
        self.member_names()
            .into_iter()
            .map(|name| pvc_name(&name))
            .collect()
    }

    pub(super) fn pv_selector(&self) -> String {
        format!("{}={}", manager::LABEL_CLUSTER, self.name)
    }
}

/// ClusterMember represents a primary or replica member in the cluster topology.
pub(super) struct ClusterMember {
    name: String,
    host_path: String,
    slot_name: Option<String>,
}

impl ClusterMember {
    fn primary(name: String) -> Self {
        Self {
            name,
            host_path: PRIMARY_HOST_PATH.to_string(),
            slot_name: None,
        }
    }

    fn replica(cluster_name: &str, ordinal: u32) -> Self {
        let name = replica_name(cluster_name, ordinal);
        Self {
            name,
            host_path: format!("{}{}", REPLICA_HOST_PATH_PREFIX, ordinal),
            slot_name: Some(format!("{}{}", REPLICA_NAME_SEGMENT, ordinal)),
        }
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn host_path(&self) -> &str {
        &self.host_path
    }

    pub(super) fn slot_name(&self) -> Option<&str> {
        self.slot_name.as_deref()
    }

    pub(super) fn pv_name(&self) -> String {
        pv_name(&self.name)
    }

    pub(super) fn pvc_name(&self) -> String {
        pvc_name(&self.name)
    }
}

pub(super) fn replica_ordinal(cluster_name: &str, resource_name: &str) -> Option<u32> {
    let prefix = format!("{}-{}-", cluster_name, REPLICA_NAME_SEGMENT);
    resource_name
        .strip_prefix(&prefix)
        .and_then(|suffix| suffix.split('-').next())
        .and_then(|ordinal| ordinal.parse::<u32>().ok())
}

fn replica_name(cluster_name: &str, ordinal: u32) -> String {
    format!("{}-{}-{}", cluster_name, REPLICA_NAME_SEGMENT, ordinal)
}

pub(super) fn pv_name(resource_name: &str) -> String {
    format!("{}-{}", resource_name, PV_NAME_SUFFIX)
}

pub(super) fn pvc_name(resource_name: &str) -> String {
    format!("{}-{}", resource_name, PVC_NAME_SUFFIX)
}
