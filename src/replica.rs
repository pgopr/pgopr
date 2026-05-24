/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use crate::workload::{self, DeploymentConfig};
use k8s_openapi::api::core::v1::ConfigMapVolumeSource;
use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{
            Container, ContainerPort, EnvVar, PersistentVolumeClaimVolumeSource, PodSpec,
            PodTemplateSpec, Volume, VolumeMount,
        },
    },
    apimachinery::pkg::apis::meta::v1::LabelSelector,
};
use kube::api::ObjectMeta;
use std::collections::BTreeMap;

/// Builds a replica deployment object
///
/// # Arguments
/// - `name` - Name of the deployment
/// - `primary_name` - Name of the primary deployment
/// - `namespace` - Namespace
/// - `slot_name` - The replication slot name
/// - `config` - Deployment configuration
pub fn build(
    name: &str,
    primary_name: &str,
    namespace: &str,
    slot_name: &str,
    config: DeploymentConfig,
) -> Deployment {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), name.to_owned());
    labels.insert("role".to_owned(), "replica".to_owned());

    // setup annotations for rolling restarts
    let mut annotations: BTreeMap<String, String> = BTreeMap::new();
    if let Some(hash) = config.config_hash {
        annotations.insert(workload::HASH_CONFIG.to_string(), hash.to_string());
    }

    let k8s_resources = config.resources.map(workload::map_resources);

    // setup volumes (pvc + optional config map)
    let mut volumes = vec![Volume {
        name: workload::DATA_VOLUME.to_string(),
        persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
            claim_name: format!("{}-pv-claim", name),
            ..Default::default()
        }),
        ..Default::default()
    }];

    let mut volume_mounts = vec![VolumeMount {
        name: workload::DATA_VOLUME.to_string(),
        mount_path: workload::DATA_MOUNT.to_string(),
        ..Default::default()
    }];

    if let Some(cm_name) = config.config_map_name {
        volumes.push(Volume {
            name: workload::CONFIG_VOLUME.to_string(),
            config_map: Some(ConfigMapVolumeSource {
                name: cm_name.to_string(),
                ..Default::default()
            }),
            ..Default::default()
        });
        volume_mounts.push(VolumeMount {
            name: workload::CONFIG_VOLUME.to_string(),
            mount_path: workload::CONFIG_MOUNT.to_string(),
            sub_path: Some("postgresql.conf".to_string()),
            ..Default::default()
        });
    }

    // Definition of the deployment
    Deployment {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(namespace.to_owned()),
            labels: Some(labels.clone()),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(1i32),
            selector: LabelSelector {
                match_labels: Some(labels.clone()),
                ..Default::default()
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels),
                    annotations: (!annotations.is_empty()).then_some(annotations),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: name.to_owned(),
                        image: Some(config.image.to_string()),
                        image_pull_policy: Some("IfNotPresent".to_string()),
                        resources: k8s_resources,
                        volume_mounts: Some(volume_mounts),
                        args: config.config_map_name.map(|_| {
                            vec![
                                "-c".into(),
                                "config_file=/etc/postgresql/postgresql.conf".into(),
                            ]
                        }),
                        ports: Some(vec![ContainerPort {
                            container_port: 5432,
                            ..Default::default()
                        }]),
                        env: Some(vec![
                            EnvVar {
                                name: "PG_PRIMARY".to_string(),
                                value: Some(primary_name.to_string()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "PG_REPLICATION_NAME".to_string(),
                                value: Some("repl_user".to_string()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "PG_REPLICATION_PASSWORD".to_string(),
                                value: Some("repl_pass".to_string()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "PG_SLOT_NAME".to_string(),
                                value: Some(slot_name.to_string()),
                                ..Default::default()
                            },
                        ]),
                        ..Default::default()
                    }],
                    volumes: Some(volumes),
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}
