/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
const PRIMARY_IMAGE: &str = "pgsql18-primary-rocky10";

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
pub fn build(name: &str, namespace: &str) -> Deployment {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), name.to_owned());
    labels.insert("role".to_owned(), "primary".to_owned());

    // Definition of the deployment
    Deployment {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(namespace.to_owned()),
            labels: Some(labels.clone()),
            ..ObjectMeta::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(1i32),
            selector: LabelSelector {
                match_expressions: None,
                match_labels: Some(labels.clone()),
            },
            template: PodTemplateSpec {
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: name.to_owned(),
                        image: Some(PRIMARY_IMAGE.to_string()),
                        image_pull_policy: Some("IfNotPresent".to_string()),
                        ports: Some(vec![ContainerPort {
                            container_port: 5432,
                            ..ContainerPort::default()
                        }]),
                        env: Some(vec![
                            EnvVar {
                                name: "PG_DATABASE".to_string(),
                                value: Some("mydb".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_USER_NAME".to_string(),
                                value: Some("myuser".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_USER_PASSWORD".to_string(),
                                value: Some("mypass".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_REPLICATION_NAME".to_string(),
                                value: Some("repl_user".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_REPLICATION_PASSWORD".to_string(),
                                value: Some("repl_pass".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_BACKUP_NAME".to_string(),
                                value: Some("backup_user".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_BACKUP_PASSWORD".to_string(),
                                value: Some("backup_pass".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_BACKUP_SLOT".to_string(),
                                value: Some("backup".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_NETWORK_MASK".to_string(),
                                value: Some("all".to_string()),
                                ..EnvVar::default()
                            },
                        ]),
                        volume_mounts: Some(vec![VolumeMount {
                            name: "mydb".to_string(),
                            mount_path: "/pgdata".to_string(),
                            ..VolumeMount::default()
                        }]),
                        ..Container::default()
                    }],
                    volumes: Some(vec![Volume {
                        name: "mydb".to_string(),
                        persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                            claim_name: format!("{}-pv-claim", name),
                            ..PersistentVolumeClaimVolumeSource::default()
                        }),
                        ..Volume::default()
                    }]),
                    ..PodSpec::default()
                }),
                metadata: Some(ObjectMeta {
                    labels: Some(labels.clone()),
                    ..ObjectMeta::default()
                }),
            },
            ..DeploymentSpec::default()
        }),
        ..Deployment::default()
    }
}
