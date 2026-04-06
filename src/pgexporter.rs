/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
const PGEXPORTER_IMAGE: &str = "pgexporter-rocky10";

use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec},
    },
    apimachinery::pkg::apis::meta::v1::LabelSelector,
};
use kube::{
    Api, Client, Error,
    api::{DeleteParams, ObjectMeta, PostParams},
};
use log::{info, trace};
use std::collections::BTreeMap;
use std::fs;

/// Creates a pgexporter deployment
///
/// # Arguments
/// - `client` - A Kubernetes client to create the deployment with
/// - `name` - Name of the deployment to be created
/// - `primary_name` - Name of the primary to monitor
/// - `namespace` - Namespace to create the Kubernetes Deployment in
///
/// Note: It is assumed the resource does not already exists for simplicity. Returns an `Error` if it does
pub async fn pgexporter_deploy(
    client: Client,
    name: &str,
    primary_name: &str,
    namespace: &str,
) -> Result<Deployment, Error> {
    // Definition of the deployment
    let deployment: Deployment = pgexporter_create(name, primary_name, namespace);
    trace!("d: {:?}", deployment);

    // Create the deployment defined above
    let deployment_api: Api<Deployment> = Api::namespaced(client, namespace);
    match deployment_api
        .create(&PostParams::default(), &deployment)
        .await
    {
        Ok(o) => {
            info!("Created pgexporter");
            Ok(o)
        }
        Err(e) => Err(e),
    }
}

/// Deletes an existing pgexporter deployment.
///
/// # Arguments:
/// - `client` - A Kubernetes client to delete the Deployment with
/// - `name` - Name of the deployment to delete
/// - `namespace` - Namespace the existing deployment resides in
///
/// Note: It is assumed the deployment exists for simplicity. Otherwise returns an Error.
pub async fn pgexporter_undeploy(client: Client, name: &str, namespace: &str) -> Result<(), Error> {
    let api: Api<Deployment> = Api::namespaced(client, namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => {
            info!("Deleted pgexporter");
        }

        Err(e) => return Err(e),
    }
    Ok(())
}

/// pgexporter: Generate
pub fn pgexporter_generate() {
    let data = serde_yaml::to_string(&pgexporter_create(
        "postgresql-pgexporter",
        "postgresql",
        "default",
    ))
    .expect("Can't serialize pgopr-pgexporter.yaml");
    fs::write("pgopr-pgexporter.yaml", data).expect("Unable to write file: pgopr-pgexporter.yaml");
}

fn pgexporter_create(name: &str, primary_name: &str, namespace: &str) -> Deployment {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), name.to_owned());

    // Definition of the deployment
    let deployment: Deployment = Deployment {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(namespace.to_owned()),
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
                        image: Some(PGEXPORTER_IMAGE.to_string()),
                        image_pull_policy: Some("IfNotPresent".to_string()),
                        ports: Some(vec![ContainerPort {
                            container_port: 5002,
                            ..ContainerPort::default()
                        }]),
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
                                value: Some("pgexporter".to_string()),
                                ..EnvVar::default()
                            },
                        ]),
                        ..Container::default()
                    }],
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
    };

    deployment
}
