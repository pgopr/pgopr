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

/// Builds a primary deployment object
///
/// # Arguments
/// - `name` - Name of the deployment
/// - `namespace` - Namespace
/// - `config` - Deployment configuration
pub fn build(name: &str, namespace: &str, config: DeploymentConfig) -> Deployment {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), name.to_owned());
    labels.insert("role".to_owned(), "primary".to_owned());

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
                                name: "PG_DATABASE".to_string(),
                                value: Some("mydb".to_string()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "PG_USER_NAME".to_string(),
                                value: Some("myuser".to_string()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "PG_USER_PASSWORD".to_string(),
                                value: Some("mypass".to_string()),
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
                                name: "PG_BACKUP_NAME".to_string(),
                                value: Some("backup_user".to_string()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "PG_BACKUP_PASSWORD".to_string(),
                                value: Some("backup_pass".to_string()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "PG_BACKUP_SLOT".to_string(),
                                value: Some("backup".to_string()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "PG_NETWORK_MASK".to_string(),
                                value: Some("all".to_string()),
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
    use crate::crd::v1::ResourceRequirements;
    use std::collections::BTreeMap;

    fn config() -> DeploymentConfig<'static> {
        DeploymentConfig {
            image: workload::PG18_PRIMARY_IMAGE,
            resources: None,
            config_map_name: None,
            config_hash: None,
        }
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
        let deployment = build("postgresql", "default", config());

        assert_eq!(deployment.metadata.name.as_deref(), Some("postgresql"));
        assert_eq!(deployment.metadata.namespace.as_deref(), Some("default"));

        let labels = deployment.metadata.labels.as_ref().unwrap();
        assert_eq!(labels.get("app").map(String::as_str), Some("postgresql"));
        assert_eq!(labels.get("role").map(String::as_str), Some("primary"));
    }

    #[test]
    fn test_build_uses_one_replica_and_matching_selector() {
        let deployment = build("postgresql", "default", config());
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
        let deployment = build("postgresql", "default", config());

        let volumes = pod(&deployment).volumes.as_ref().unwrap();
        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].name, workload::DATA_VOLUME);
        assert_eq!(
            volumes[0]
                .persistent_volume_claim
                .as_ref()
                .unwrap()
                .claim_name,
            "postgresql-pv-claim"
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
    fn test_build_sets_primary_environment() {
        let deployment = build("postgresql", "default", config());

        assert_eq!(
            env_value(&deployment, "PG_DATABASE").as_deref(),
            Some("mydb")
        );
        assert_eq!(
            env_value(&deployment, "PG_USER_NAME").as_deref(),
            Some("myuser")
        );
        assert_eq!(
            env_value(&deployment, "PG_REPLICATION_NAME").as_deref(),
            Some("repl_user")
        );
        assert_eq!(
            env_value(&deployment, "PG_BACKUP_SLOT").as_deref(),
            Some("backup")
        );
        assert_eq!(
            env_value(&deployment, "PG_NETWORK_MASK").as_deref(),
            Some("all")
        );
    }

    #[test]
    fn test_build_uses_image_and_port() {
        let deployment = build("postgresql", "default", config());
        let container = container(&deployment);

        assert_eq!(
            container.image.as_deref(),
            Some(workload::PG18_PRIMARY_IMAGE)
        );
        assert_eq!(container.ports.as_ref().unwrap()[0].container_port, 5432);
    }

    #[test]
    fn test_build_without_config_map_has_no_config_volume_args_or_hash() {
        let deployment = build("postgresql", "default", config());

        assert_eq!(pod(&deployment).volumes.as_ref().unwrap().len(), 1);
        assert!(container(&deployment).args.is_none());
        assert!(
            deployment
                .spec
                .as_ref()
                .unwrap()
                .template
                .metadata
                .as_ref()
                .unwrap()
                .annotations
                .is_none()
        );
    }

    #[test]
    fn test_build_with_config_map_adds_volume_mount_and_args() {
        let config = DeploymentConfig {
            config_map_name: Some("postgresql-config"),
            ..config()
        };
        let deployment = build("postgresql", "default", config);

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
    }

    #[test]
    fn test_build_sets_config_hash_annotation() {
        let config = DeploymentConfig {
            config_hash: Some("abc123"),
            ..config()
        };
        let deployment = build("postgresql", "default", config);

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

    #[test]
    fn test_build_maps_resources() {
        let resources = ResourceRequirements {
            limits: Some(BTreeMap::from([
                ("cpu".to_string(), "1".to_string()),
                ("memory".to_string(), "1Gi".to_string()),
            ])),
            requests: Some(BTreeMap::from([("cpu".to_string(), "500m".to_string())])),
        };
        let config = DeploymentConfig {
            resources: Some(&resources),
            ..config()
        };
        let deployment = build("postgresql", "default", config);
        let mapped = container(&deployment).resources.as_ref().unwrap();

        let limits = mapped.limits.as_ref().unwrap();
        assert_eq!(limits.get("cpu").unwrap().0, "1");
        assert_eq!(limits.get("memory").unwrap().0, "1Gi");
        let requests = mapped.requests.as_ref().unwrap();
        assert_eq!(requests.get("cpu").unwrap().0, "500m");
    }
}
