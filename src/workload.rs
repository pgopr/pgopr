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
