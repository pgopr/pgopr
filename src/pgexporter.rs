/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use crate::crd::v1::ResourceRequirements;
use crate::workload;
use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{
            Container, ContainerPort, EnvVar, EnvVarSource, ExecAction, PodSpec, PodTemplateSpec,
            Probe, Secret, SecretKeySelector,
        },
    },
    apimachinery::pkg::apis::meta::v1::LabelSelector,
};
use kube::api::ObjectMeta;
use std::collections::BTreeMap;
use std::fs;
/// Builds a secret containing the pgexporter password
///
/// # Arguments
/// - `name` - Name of the secret
/// - `namespace` - Namespace
/// - `backup_password` - The backup user password
pub fn build_secret(name: &str, namespace: &str, exporter_password: &str) -> Secret {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), name.to_owned());

    let mut string_data = BTreeMap::new();
    string_data.insert(
        "PG_EXPORTER_PASSWORD".to_string(),
        exporter_password.to_string(),
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

pub fn pgexporter_generate() {
    let data = serde_yaml::to_string(&build_deployment(
        "postgresql-pgexporter",
        "default",
        "postgresql",
        "postgresql-pgexporter-secret",
        None,
    ))
    .expect("Can't serialize...");
    fs::write("pgopr-pgexporter.yaml", data).expect("...");
}

/// Builds a pgexporter deployment object
///
/// # Arguments
/// - `name` - Name of the deployment
/// - `namespace` - Namespace
/// - `primary_name` - Name of the primary service for PG_PRIMARY_NAME env var
/// - `secret_name` - Name of the secret containing PG_BACKUP_PASSWORD
/// - `resources` - Name of the resources for pgexporter
pub fn build_deployment(
    name: &str,
    namespace: &str,
    primary_name: &str,
    secret_name: &str,
    resources: Option<&ResourceRequirements>,
) -> Deployment {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let k8s_resources = resources.map(workload::map_resources);
    labels.insert("app".to_owned(), name.to_owned());
    labels.insert("role".to_owned(), "exporter".to_owned());

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
                        image: Some(workload::PGEXPORTER_IMAGE.to_string()),
                        image_pull_policy: Some("IfNotPresent".to_string()),
                        ports: Some(vec![
                            ContainerPort {
                                container_port: workload::PGEXPORTER_PORT,
                                ..ContainerPort::default()
                            },
                            ContainerPort {
                                container_port: workload::PGEXPORTER_METRICS_PORT,
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
                                name: "PG_EXPORTER_NAME".to_string(),
                                value: Some("pgexporter".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PG_EXPORTER_PASSWORD".to_string(),
                                value_from: Some(EnvVarSource {
                                    secret_key_ref: Some(SecretKeySelector {
                                        name: secret_name.to_string(),
                                        key: "PG_EXPORTER_PASSWORD".to_string(),
                                        ..SecretKeySelector::default()
                                    }),
                                    ..EnvVarSource::default()
                                }),
                                ..EnvVar::default()
                            },
                        ]),
                        liveness_probe: Some(Probe {
                            initial_delay_seconds: Some(30),
                            exec: Some(ExecAction {
                                command: Some(vec![
                                    "pgexporter-cli".to_string(),
                                    "-c".to_string(),
                                    "/pgexporter/pgexporter.conf".to_string(),
                                    "ping".to_string(),
                                ]),
                            }),
                            ..Probe::default()
                        }),
                        readiness_probe: Some(Probe {
                            initial_delay_seconds: Some(15),
                            exec: Some(ExecAction {
                                command: Some(vec![
                                    "pgexporter-cli".to_string(),
                                    "-c".to_string(),
                                    "/pgexporter/pgexporter.conf".to_string(),
                                    "ping".to_string(),
                                ]),
                            }),
                            ..Probe::default()
                        }),
                        resources: k8s_resources,
                        ..Container::default()
                    }],
                    ..PodSpec::default()
                }),
            },
            ..DeploymentSpec::default()
        }),
        ..Deployment::default()
    }
}

/// Builds a pgexporter monitoring deployment object (Grafana + Prometheus)
///
/// # Arguments
/// - `name` - Name of the deployment
/// - `namespace` - Namespace
/// - `service_host` - Name of the pgexporter service for PGEXPORTER_SERVICE_HOST env var
/// - `resources` - Name of the resources for pgexporter-mon
pub fn build_monitoring_deployment(
    name: &str,
    namespace: &str,
    service_host: &str,
    resources: Option<&ResourceRequirements>,
) -> Deployment {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let k8s_resources = resources.map(workload::map_resources);
    labels.insert("app".to_owned(), name.to_owned());

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
                        image: Some(workload::PGEXPORTER_MON_IMAGE.to_string()),
                        image_pull_policy: Some("IfNotPresent".to_string()),
                        ports: Some(vec![
                            ContainerPort {
                                container_port: workload::PGEXPORTER_MON_GRAFANA_PORT,
                                ..ContainerPort::default()
                            },
                            ContainerPort {
                                container_port: workload::PGEXPORTER_METRICS_PORT,
                                ..ContainerPort::default()
                            },
                            ContainerPort {
                                container_port: workload::PGEXPORTER_MON_PROMETHEUS_PORT,
                                ..ContainerPort::default()
                            },
                        ]),
                        env: Some(vec![
                            EnvVar {
                                name: "PGEXPORTER_SERVICE_HOST".to_string(),
                                value: Some(service_host.to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "PGEXPORTER_SERVICE_PORT".to_string(),
                                value: Some("5002".to_string()),
                                ..EnvVar::default()
                            },
                        ]),
                        liveness_probe: Some(Probe {
                            initial_delay_seconds: Some(30),
                            exec: Some(ExecAction {
                                command: Some(vec![
                                    "curl".to_string(),
                                    "-f".to_string(),
                                    "http://localhost:3000/api/health".to_string(),
                                ]),
                            }),
                            ..Probe::default()
                        }),
                        resources: k8s_resources,
                        ..Container::default()
                    }],
                    ..PodSpec::default()
                }),
            },
            ..DeploymentSpec::default()
        }),
        ..Deployment::default()
    }
}
