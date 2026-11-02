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

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> DeploymentConfig<'static> {
        DeploymentConfig {
            image: workload::PG18_REPLICA_IMAGE,
            resources: None,
            config_map_name: None,
            config_hash: None,
        }
    }

    fn build_replica(config: DeploymentConfig) -> Deployment {
        build("postgresql-1", "postgresql", "default", "replica_1", config)
    }

    fn pod(deployment: &Deployment) -> &PodSpec {
        deployment
            .spec
            .as_ref()
            .expect("deployment spec")
            .template
            .spec
            .as_ref()
            .expect("pod spec")
    }

    fn container(deployment: &Deployment) -> &Container {
        &pod(deployment).containers[0]
    }

    fn env_value(deployment: &Deployment, name: &str) -> Option<String> {
        container(deployment)
            .env
            .as_ref()?
            .iter()
            .find(|env| env.name == name)
            .and_then(|env| env.value.clone())
    }

    #[test]
    fn test_build_sets_metadata_and_role_labels() {
        let deployment = build_replica(config());

        assert_eq!(deployment.metadata.name.as_deref(), Some("postgresql-1"));
        assert_eq!(deployment.metadata.namespace.as_deref(), Some("default"));

        let labels = deployment.metadata.labels.as_ref().unwrap();
        assert_eq!(labels.get("app").map(String::as_str), Some("postgresql-1"));
        assert_eq!(labels.get("role").map(String::as_str), Some("replica"));
    }

    #[test]
    fn test_build_uses_one_replica_and_matching_selector() {
        let deployment = build_replica(config());
        let spec = deployment.spec.as_ref().unwrap();

        assert_eq!(spec.replicas, Some(1));
        assert_eq!(
            spec.selector.match_labels.as_ref().unwrap(),
            spec.template
                .metadata
                .as_ref()
                .unwrap()
                .labels
                .as_ref()
                .unwrap()
        );
    }

    #[test]
    fn test_build_mounts_data_volume_claim() {
        let deployment = build_replica(config());

        let volumes = pod(&deployment).volumes.as_ref().unwrap();
        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].name, workload::DATA_VOLUME);
        assert_eq!(
            volumes[0]
                .persistent_volume_claim
                .as_ref()
                .unwrap()
                .claim_name,
            "postgresql-1-pv-claim"
        );

        let mount = container(&deployment)
            .volume_mounts
            .as_ref()
            .unwrap()
            .iter()
            .find(|mount| mount.name == workload::DATA_VOLUME)
            .expect("data volume mount");
        assert_eq!(mount.mount_path, workload::DATA_MOUNT);
    }

    #[test]
    fn test_build_sets_primary_and_slot_environment() {
        let deployment = build_replica(config());

        assert_eq!(
            env_value(&deployment, "PG_PRIMARY").as_deref(),
            Some("postgresql")
        );
        assert_eq!(
            env_value(&deployment, "PG_SLOT_NAME").as_deref(),
            Some("replica_1")
        );
        assert_eq!(
            env_value(&deployment, "PG_REPLICATION_NAME").as_deref(),
            Some("repl_user")
        );
    }

    #[test]
    fn test_build_uses_image_and_port() {
        let deployment = build_replica(config());
        let container = container(&deployment);

        assert_eq!(
            container.image.as_deref(),
            Some(workload::PG18_REPLICA_IMAGE)
        );
        assert_eq!(container.ports.as_ref().unwrap()[0].container_port, 5432);
    }

    #[test]
    fn test_build_with_config_map_adds_volume_mount_and_args() {
        let deployment = build_replica(DeploymentConfig {
            config_map_name: Some("postgresql-config"),
            config_hash: Some("abc123"),
            ..config()
        });

        let volumes = pod(&deployment).volumes.as_ref().unwrap();
        assert_eq!(volumes.len(), 2);
        let config_volume = volumes
            .iter()
            .find(|volume| volume.name == workload::CONFIG_VOLUME)
            .expect("config volume");
        assert_eq!(
            config_volume.config_map.as_ref().unwrap().name,
            "postgresql-config"
        );

        let mount = container(&deployment)
            .volume_mounts
            .as_ref()
            .unwrap()
            .iter()
            .find(|mount| mount.name == workload::CONFIG_VOLUME)
            .expect("config volume mount");
        assert_eq!(mount.mount_path, workload::CONFIG_MOUNT);
        assert_eq!(mount.sub_path.as_deref(), Some("postgresql.conf"));

        assert_eq!(
            container(&deployment).args.as_ref().unwrap(),
            &vec![
                "-c".to_string(),
                "config_file=/etc/postgresql/postgresql.conf".to_string()
            ]
        );

        let annotations = deployment
            .spec
            .as_ref()
            .unwrap()
            .template
            .metadata
            .as_ref()
            .unwrap()
            .annotations
            .as_ref()
            .unwrap();
        assert_eq!(
            annotations.get(workload::HASH_CONFIG).map(String::as_str),
            Some("abc123")
        );
    }
}
