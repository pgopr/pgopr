/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use crate::crd::v1::ResourceRequirements;
use k8s_openapi::api::core::v1::ResourceRequirements as K8sResources;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

pub const PG18_PRIMARY_IMAGE: &str = "pgsql18-primary-rocky10";
pub const PG18_REPLICA_IMAGE: &str = "pgsql18-replica-rocky10";
pub const HASH_CONFIG: &str = "pgopr.io/config-hash";
pub const DATA_VOLUME: &str = "pgdata";
pub const DATA_MOUNT: &str = "/pgdata";
pub const CONFIG_VOLUME: &str = "config";
pub const CONFIG_MOUNT: &str = "/etc/postgresql/postgresql.conf";
pub const PGMONETA_IMAGE: &str = "pgmoneta-rocky10";
pub const PGMONETA_PORT: i32 = 5001;
pub const PGMONETA_METRICS_PORT: i32 = 9100;

pub const PGEXPORTER_IMAGE: &str = "pgexporter-rocky10";
pub const PGEXPORTER_PORT: i32 = 5002;
pub const PGEXPORTER_METRICS_PORT: i32 = 9100;
/// monitoring image for pgexporter stack(grafana, prometheus)
pub const PGEXPORTER_MON_IMAGE: &str = "grafana-rocky10";
pub const PGEXPORTER_MON_GRAFANA_PORT: i32 = 3000;
pub const PGEXPORTER_MON_PROMETHEUS_PORT: i32 = 9090;

pub struct DeploymentConfig<'a> {
    pub image: &'static str,
    pub resources: Option<&'a ResourceRequirements>,
    pub config_map_name: Option<&'a str>,
    pub config_hash: Option<&'a str>,
}

pub fn map_resources(reqs: &ResourceRequirements) -> K8sResources {
    let mut k8s_reqs = K8sResources::default();

    if let Some(limits) = &reqs.limits {
        k8s_reqs.limits = Some(
            limits
                .iter()
                .map(|(k, v)| (k.clone(), Quantity(v.clone())))
                .collect(),
        );
    }

    if let Some(requests) = &reqs.requests {
        k8s_reqs.requests = Some(
            requests
                .iter()
                .map(|(k, v)| (k.clone(), Quantity(v.clone())))
                .collect(),
        );
    }

    k8s_reqs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_map_resources_keeps_absent_sections_absent() {
        let reqs = ResourceRequirements {
            limits: None,
            requests: None,
        };
        let mapped = map_resources(&reqs);

        assert!(mapped.limits.is_none());
        assert!(mapped.requests.is_none());
    }

    #[test]
    fn test_map_resources_maps_limits_and_requests() {
        let reqs = ResourceRequirements {
            limits: Some(BTreeMap::from([
                ("cpu".to_string(), "1".to_string()),
                ("memory".to_string(), "1Gi".to_string()),
            ])),
            requests: Some(BTreeMap::from([("cpu".to_string(), "500m".to_string())])),
        };
        let mapped = map_resources(&reqs);

        let limits = mapped.limits.as_ref().unwrap();
        assert_eq!(limits.get("cpu").unwrap().0, "1");
        assert_eq!(limits.get("memory").unwrap().0, "1Gi");
        let requests = mapped.requests.as_ref().unwrap();
        assert_eq!(requests.get("cpu").unwrap().0, "500m");
    }

    #[test]
    fn test_map_resources_preserves_empty_maps() {
        let reqs = ResourceRequirements {
            limits: Some(BTreeMap::new()),
            requests: Some(BTreeMap::new()),
        };
        let mapped = map_resources(&reqs);

        assert!(mapped.limits.as_ref().unwrap().is_empty());
        assert!(mapped.requests.as_ref().unwrap().is_empty());
    }
}
