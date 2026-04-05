/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
const REPLICA_IMAGE: &str = "pgsql18-replica-rocky10";

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
    Api, Client, Error,
    api::{DeleteParams, ObjectMeta, PostParams},
};
use log::{info, trace};
use std::collections::BTreeMap;
use std::fs;

/// Creates a replica deployment
///
/// # Arguments
/// - `client` - A Kubernetes client to create the deployment with
/// - `name` - Name of the deployment to be created
/// - `namespace` - Namespace to create the Kubernetes Deployment in
///
/// Note: It is assumed the resource does not already exists for simplicity. Returns an `Error` if it does
pub async fn replica_deploy(
    client: Client,
    name: &str,
    primary_name: &str,
    namespace: &str,
    slot_name: &str,
) -> Result<Deployment, Error> {
    // Definition of the deployment
    let deployment: Deployment = replica_create(name, primary_name, namespace, slot_name);
    trace!("d: {:?}", deployment);

    // Create the deployment defined above
    let deployment_api: Api<Deployment> = Api::namespaced(client, namespace);
    match deployment_api
        .create(&PostParams::default(), &deployment)
        .await
    {
        Ok(o) => {
            info!("Created Replica");
            Ok(o)
        }
        Err(e) => Err(e),
    }
}

/// Deletes an existing replica deployment.
///
/// # Arguments:
/// - `client` - A Kubernetes client to delete the Deployment with
/// - `name` - Name of the deployment to delete
/// - `namespace` - Namespace the existing deployment resides in
///
/// Note: It is assumed the deployment exists for simplicity. Otherwise returns an Error.
pub async fn replica_undeploy(client: Client, name: &str, namespace: &str) -> Result<(), Error> {
    let api: Api<Deployment> = Api::namespaced(client, namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => {
            info!("Deleted Replica");
        }

        Err(e) => return Err(e),
    }
    Ok(())
}

/// Replica: Generate
pub fn replica_generate() {
    let data = serde_yaml::to_string(&replica_create(
        "postgresql-replica",
        "postgresql",
        "default",
        "replica1",
    ))
    .expect("Can't serialize pgopr-replica.yaml");
    fs::write("pgopr-replica.yaml", data).expect("Unable to write file: pgopr-replica.yaml");
}

fn replica_create(name: &str, primary_name: &str, namespace: &str, slot_name: &str) -> Deployment {
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
                        image: Some(REPLICA_IMAGE.to_string()),
                        image_pull_policy: Some("IfNotPresent".to_string()),
                        ports: Some(vec![ContainerPort {
                            container_port: 5432,
                            ..ContainerPort::default()
                        }]),
                        env: Some(vec![
                            EnvVar {
                                name: "PG_PRIMARY".to_string(),
                                value: Some(primary_name.to_string()),
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
                                name: "PG_SLOT_NAME".to_string(),
                                value: Some(slot_name.to_string()),
                                ..EnvVar::default()
                            },
                        ]),
                        volume_mounts: Some(vec![VolumeMount {
                            name: "mydb-replica".to_string(),
                            mount_path: "/pgdata".to_string(),
                            ..VolumeMount::default()
                        }]),
                        ..Container::default()
                    }],
                    volumes: Some(vec![Volume {
                        name: "mydb-replica".to_string(),
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
    };

    deployment
}
