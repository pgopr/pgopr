/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use crate::workload;
use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{
            Container, ContainerPort, EnvVar, EnvVarSource, ExecAction,
            PersistentVolumeClaimVolumeSource, PodSpec, PodTemplateSpec, Probe, Secret,
            SecretKeySelector, Volume, VolumeMount,
        },
    },
    apimachinery::pkg::apis::meta::v1::LabelSelector,
};
use kube::api::ObjectMeta;
use std::collections::BTreeMap;

/// Builds a secret containing the pgmoneta backup password
///
/// # Arguments
/// - `name` - Name of the secret
/// - `namespace` - Namespace
/// - `backup_password` - The backup user password
pub fn build_secret(name: &str, namespace: &str, backup_password: &str) -> Secret {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), name.to_owned());

    let mut string_data = BTreeMap::new();
    string_data.insert(
        "PG_BACKUP_PASSWORD".to_string(),
        backup_password.to_string(),
    );

    Secret {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(namespace.to_owned()),
            labels: Some(labels),
            ..ObjectMeta::default()
        },
        string_data: Some(string_data),
        ..Secret::default()
    }
}

/// Builds a pgmoneta deployment object
///
/// # Arguments
/// - `name` - Name of the deployment
/// - `namespace` - Namespace
/// - `primary_name` - Name of the primary service for PG_PRIMARY_NAME env var
/// - `pvc_name` - Name of the PVC to mount at /home/pgmoneta
/// - `secret_name` - Name of the secret containing PG_BACKUP_PASSWORD
pub fn build_deployment(
    name: &str,
    namespace: &str,
    primary_name: &str,
    pvc_name: &str,
    secret_name: &str,
) -> Deployment {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), name.to_owned());
    labels.insert("role".to_owned(), "backup".to_owned());

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
                match_labels: Some(labels.clone()),
                ..LabelSelector::default()
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels),
                    ..ObjectMeta::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: name.to_owned(),
                        image: Some(workload::PGMONETA_IMAGE.to_string()),
                        image_pull_policy: Some("IfNotPresent".to_string()),
                        ports: Some(vec![
                            ContainerPort {
                                container_port: workload::PGMONETA_PORT,
                                ..ContainerPort::default()
                            },
                            ContainerPort {
                                container_port: workload::PGMONETA_METRICS_PORT,
                                ..ContainerPort::default()
                            },
                        ]),
                        env: Some(vec![
                            EnvVar {
                                name: "PG_PRIMARY_NAME".to_string(),
                                value: Some(primary_name.to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_PRIMARY_PORT".to_string(),
                                value: Some("5432".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_BACKUP_NAME".to_string(),
                                value: Some("backup_user".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_BACKUP_PASSWORD".to_string(),
                                value_from: Some(EnvVarSource {
                                    secret_key_ref: Some(SecretKeySelector {
                                        name: secret_name.to_string(),
                                        key: "PG_BACKUP_PASSWORD".to_string(),
                                        ..SecretKeySelector::default()
                                    }),
                                    ..EnvVarSource::default()
                                }),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_BACKUP_SLOT".to_string(),
                                value: Some("backup".to_string()),
                                ..EnvVar::default()
                            },
                        ]),
                        volume_mounts: Some(vec![VolumeMount {
                            name: "pgmoneta-data".to_string(),
                            mount_path: "/home/pgmoneta".to_string(),
                            ..VolumeMount::default()
                        }]),
                        liveness_probe: Some(Probe {
                            initial_delay_seconds: Some(30),
                            exec: Some(ExecAction {
                                command: Some(vec![
                                    "pgmoneta-cli".to_string(),
                                    "-c".to_string(),
                                    "/pgmoneta/pgmoneta.conf".to_string(),
                                    "ping".to_string(),
                                ]),
                            }),
                            ..Probe::default()
                        }),
                        readiness_probe: Some(Probe {
                            initial_delay_seconds: Some(15),
                            exec: Some(ExecAction {
                                command: Some(vec![
                                    "pgmoneta-cli".to_string(),
                                    "-c".to_string(),
                                    "/pgmoneta/pgmoneta.conf".to_string(),
                                    "ping".to_string(),
                                ]),
                            }),
                            ..Probe::default()
                        }),
                        ..Container::default()
                    }],
                    volumes: Some(vec![Volume {
                        name: "pgmoneta-data".to_string(),
                        persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                            claim_name: pvc_name.to_string(),
                            ..PersistentVolumeClaimVolumeSource::default()
                        }),
                        ..Volume::default()
                    }]),
                    ..PodSpec::default()
                }),
            },
            ..DeploymentSpec::default()
        }),
        ..Deployment::default()
    }
}
