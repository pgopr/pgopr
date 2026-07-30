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

/// pgmoenta is a special resource type that is used to store pgmoneta data.
const PGMONETA_SUFFIX: &str = "pgmoneta";
const PGMONETA_PV_NAME_SUFFIX: &str = "pgmoneta-pv-volume";
const PGMONETA_PVC_NAME_SUFFIX: &str = "pgmoneta-pv-claim";
const PGMONETA_SECRET_SUFFIX: &str = "pgmoneta-secret";

/// pgexporter is a special resource type that is used to store pgexporter data.
const PGEXPORTER_SUFFIX: &str = "pgexporter";
const PGEXPORTER_SECRET_SUFFIX: &str = "pgexporter-secret";
const PGEXPORTER_MON_SUFFIX: &str = "pgexporter-mon";

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

    pub fn pgmoneta_name(&self) -> String {
        format!("{}-{}", self.name, PGMONETA_SUFFIX)
    }
    pub fn pgmoneta_pv_name(&self) -> String {
        format!("{}-{}", self.name, PGMONETA_PV_NAME_SUFFIX)
    }
    pub fn pgmoneta_pvc_name(&self) -> String {
        format!("{}-{}", self.name, PGMONETA_PVC_NAME_SUFFIX)
    }
    pub fn pgmoneta_secret_name(&self) -> String {
        format!("{}-{}", self.name, PGMONETA_SECRET_SUFFIX)
    }

    pub fn pgexporter_name(&self) -> String {
        format!("{}-{}", self.name, PGEXPORTER_SUFFIX)
    }
    pub fn pgexporter_secret_name(&self) -> String {
        format!("{}-{}", self.name, PGEXPORTER_SECRET_SUFFIX)
    }

    pub fn pgexporter_mon_name(&self) -> String {
        format!("{}-{}", self.name, PGEXPORTER_MON_SUFFIX)
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── ClusterTopology naming helpers ──

    #[test]
    fn topology_pgmoneta_name() {
        let t = ClusterTopology {
            name: "test".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 0,
        };
        assert_eq!(t.pgmoneta_name(), "test-pgmoneta");
    }

    #[test]
    fn topology_pgmoneta_pv_name() {
        let t = ClusterTopology {
            name: "test".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 0,
        };
        assert_eq!(t.pgmoneta_pv_name(), "test-pgmoneta-pv-volume");
    }

    #[test]
    fn topology_pgmoneta_pvc_name() {
        let t = ClusterTopology {
            name: "test".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 0,
        };
        assert_eq!(t.pgmoneta_pvc_name(), "test-pgmoneta-pv-claim");
    }

    #[test]
    fn topology_pgmoneta_secret_name() {
        let t = ClusterTopology {
            name: "test".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 0,
        };
        assert_eq!(t.pgmoneta_secret_name(), "test-pgmoneta-secret");
    }

    #[test]
    fn topology_pgexporter_name() {
        let t = ClusterTopology {
            name: "test".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 0,
        };
        assert_eq!(t.pgexporter_name(), "test-pgexporter");
    }

    #[test]
    fn topology_pgexporter_secret_name() {
        let t = ClusterTopology {
            name: "test".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 0,
        };
        assert_eq!(t.pgexporter_secret_name(), "test-pgexporter-secret");
    }

    #[test]
    fn topology_pgexporter_mon_name() {
        let t = ClusterTopology {
            name: "test".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 0,
        };
        assert_eq!(t.pgexporter_mon_name(), "test-pgexporter-mon");
    }

    // ── replica_ordinal ──

    #[test]
    fn replica_ordinal_valid_three() {
        assert_eq!(replica_ordinal("mycluster", "mycluster-replica-3"), Some(3));
    }

    #[test]
    fn replica_ordinal_valid_one() {
        assert_eq!(replica_ordinal("mycluster", "mycluster-replica-1"), Some(1));
    }

    #[test]
    fn replica_ordinal_non_replica_label_returns_none() {
        assert_eq!(replica_ordinal("mycluster", "mycluster-primary"), None);
    }

    #[test]
    fn replica_ordinal_unrelated_name_returns_none() {
        assert_eq!(replica_ordinal("mycluster", "some-other-deployment"), None);
    }

    #[test]
    fn replica_ordinal_non_numeric_suffix_returns_none() {
        assert_eq!(replica_ordinal("mycluster", "mycluster-replica-abc"), None);
    }

    #[test]
    fn replica_ordinal_prefix_mismatch_returns_none() {
        assert_eq!(replica_ordinal("cluster-a", "cluster-b-replica-1"), None);
    }

    // ── replica_name ──

    #[test]
    fn replica_name_first_replica() {
        assert_eq!(replica_name("mycluster", 1), "mycluster-replica-1");
    }

    #[test]
    fn replica_name_tenth_replica() {
        assert_eq!(replica_name("mycluster", 10), "mycluster-replica-10");
    }

    // ── pv_name / pvc_name ──

    #[test]
    fn pv_name_appends_pv_volume_suffix() {
        assert_eq!(pv_name("postgresql"), "postgresql-pv-volume");
    }

    #[test]
    fn pvc_name_appends_pv_claim_suffix() {
        assert_eq!(
            pvc_name("postgresql-replica-2"),
            "postgresql-replica-2-pv-claim"
        );
    }

    // ── ClusterMember ──

    #[test]
    fn primary_member_has_no_slot() {
        let m = ClusterMember::primary("mycluster".into());
        assert_eq!(m.name(), "mycluster");
        assert_eq!(m.host_path(), "/tmp/kind");
        assert_eq!(m.slot_name(), None);
    }

    #[test]
    fn primary_member_pv_and_pvc_names() {
        let m = ClusterMember::primary("mycluster".into());
        assert_eq!(m.pv_name(), "mycluster-pv-volume");
        assert_eq!(m.pvc_name(), "mycluster-pv-claim");
    }

    #[test]
    fn replica_member_has_slot() {
        let m = ClusterMember::replica("mycluster", 2);
        assert_eq!(m.name(), "mycluster-replica-2");
        assert_eq!(m.host_path(), "/tmp/kind-replica-2");
        assert_eq!(m.slot_name(), Some("replica2"));
    }

    #[test]
    fn replica_member_pv_and_pvc_names() {
        let m = ClusterMember::replica("mycluster", 2);
        assert_eq!(m.pv_name(), "mycluster-replica-2-pv-volume");
        assert_eq!(m.pvc_name(), "mycluster-replica-2-pv-claim");
    }

    // ── ClusterTopology getters and members ──

    #[test]
    fn topology_storage_and_replicas() {
        let t = ClusterTopology {
            name: "c".into(),
            namespace: "ns".into(),
            storage: 10,
            replicas: 3,
        };
        assert_eq!(t.storage(), 10);
        assert_eq!(t.replicas(), 3);
    }

    #[test]
    fn topology_primary() {
        let t = ClusterTopology {
            name: "mycluster".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 0,
        };
        let p = t.primary();
        assert_eq!(p.name(), "mycluster");
    }

    #[test]
    fn topology_replica_members_count_matches_replicas() {
        let t = ClusterTopology {
            name: "c".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 3,
        };
        let members = t.replica_members();
        assert_eq!(members.len(), 3);
        assert_eq!(members[0].name(), "c-replica-1");
        assert_eq!(members[1].name(), "c-replica-2");
        assert_eq!(members[2].name(), "c-replica-3");
    }

    #[test]
    fn topology_no_replicas_when_replicas_is_zero() {
        let t = ClusterTopology {
            name: "c".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 0,
        };
        assert!(t.replica_members().is_empty());
    }

    #[test]
    fn topology_member_names_includes_primary() {
        let t = ClusterTopology {
            name: "c".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 0,
        };
        assert!(t.member_names().contains("c"));
    }

    #[test]
    fn topology_member_names_includes_replicas() {
        let t = ClusterTopology {
            name: "c".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 2,
        };
        let names = t.member_names();
        assert!(names.contains("c"));
        assert!(names.contains("c-replica-1"));
        assert!(names.contains("c-replica-2"));
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn topology_pvc_names_includes_all_members() {
        let t = ClusterTopology {
            name: "c".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 1,
        };
        let pvcs = t.pvc_names();
        assert!(pvcs.contains("c-pv-claim"));
        assert!(pvcs.contains("c-replica-1-pv-claim"));
    }

    #[test]
    fn topology_pv_selector() {
        let t = ClusterTopology {
            name: "mycluster".into(),
            namespace: "ns".into(),
            storage: 5,
            replicas: 0,
        };
        assert_eq!(t.pv_selector(), "pgopr.io/cluster=mycluster");
    }
}
