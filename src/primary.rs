/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
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
use kube::{
    api::{DeleteParams, ObjectMeta, PostParams},
    Api, Client, Error,
};
use log::{info, trace};
use std::collections::BTreeMap;
use std::fs;

/// Creates a primary deployment
///
/// # Arguments
/// - `client` - A Kubernetes client to create the deployment with
/// - `name` - Name of the deployment to be created
/// - `namespace` - Namespace to create the Kubernetes Deployment in
///
/// Note: It is assumed the resource does not already exists for simplicity. Returns an `Error` if it does
pub async fn primary_deploy(
    client: Client,
    name: &str,
    namespace: &str,
) -> Result<Deployment, Error> {
    // Definition of the deployment
    let deployment: Deployment = primary_create(name, namespace);
    trace!("d: {:?}", deployment);

    // Create the deployment defined above
    let deployment_api: Api<Deployment> = Api::namespaced(client, namespace);
    match deployment_api
        .create(&PostParams::default(), &deployment)
        .await {
        Ok(o) => {
            info!("Created Primary");
            return Ok(o);
        }
        Err(e) => return Err(e.into()),
    }
}

/// Deletes an existing primary deployment.
///
/// # Arguments:
/// - `client` - A Kubernetes client to delete the Deployment with
/// - `name` - Name of the deployment to delete
/// - `namespace` - Namespace the existing deployment resides in
///
/// Note: It is assumed the deployment exists for simplicity. Otherwise returns an Error.
pub async fn primary_undeploy(client: Client, name: &str, namespace: &str) -> Result<(), Error> {
    let api: Api<Deployment> = Api::namespaced(client, namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => {
            info!("Deleted Primary");
        }

        Err(e) => return Err(e.into()),
    }
    Ok(())
}

/// Primary: Generate
pub fn primary_generate() {
    let data = serde_yaml::to_string(&primary_create("postgresql", "default"))
        .expect("Can't serialize pgopr-primary.yaml");
    fs::write("pgopr-primary.yaml", data).expect("Unable to write file: pgopr-primary.yaml");
}

fn primary_create(name: &str, namespace: &str) -> Deployment {
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
                        image: Some("postgres:13.7".to_owned()),
                        image_pull_policy: Some("IfNotPresent".to_string()),
                        ports: Some(vec![ContainerPort {
                            container_port: 5432,
                            ..ContainerPort::default()
                        }]),
                        env: Some(vec![
                            EnvVar {
                                name: "POSTGRES_DB".to_string(),
                                value: Some("mydb".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "POSTGRES_USER".to_string(),
                                value: Some("myuser".to_string()),
                                ..EnvVar::default()
                            },
                            EnvVar {
                                name: "POSTGRES_PASSWORD".to_string(),
                                value: Some("mypass".to_string()),
                                ..EnvVar::default()
                            },
                        ]),
                        volume_mounts: Some(vec![VolumeMount {
                            name: "mydb".to_string(),
                            mount_path: "/var/lib/postgresql/data".to_string(),
                            ..VolumeMount::default()
                        }]),
                        ..Container::default()
                    }],
                    volumes: Some(vec![Volume {
                        name: "mydb".to_string(),
                        persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                            claim_name: "postgresql-pv-claim".to_string(),
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
    };

    deployment
}
