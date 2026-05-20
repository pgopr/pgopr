/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;
use kube::CustomResource;
use kube::{
    Api, Client, Error,
    api::{DeleteParams, Patch, PatchParams, PostParams},
    core::crd::CustomResourceExt,
    runtime::wait::{await_condition, conditions},
};
use log::{info, trace};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs;

pub mod v1 {
    use super::*;

    /// The CustomDefinitionResource for the operator
    #[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
    #[kube(
        group = "pgopr.io",
        version = "v1",
        kind = "pgopr",
        plural = "pgoprs",
        derive = "PartialEq",
        status = "PgOprStatus",
        namespaced
    )]
    pub struct PgOprSpec {
        /// General settings across all components
        pub storage: u32,
        /// Number of replicas in the star configuration
        pub replicas: Option<u32>,
    }

    /// The status of the PgOpr resource
    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
    pub struct PgOprStatus {
        /// Current phase (e.g., Pending, Running, Failed)
        pub phase: String,
        /// Status of the primary deployment
        pub primary: Option<DeploymentStatus>,
        /// List of replica deployment statuses
        #[serde(default)]
        pub replicas: Vec<DeploymentStatus>,
        /// List of service statuses
        #[serde(default)]
        pub services: Vec<ServiceStatus>,
        /// List of storage statuses
        #[serde(default)]
        pub storage: Vec<StorageStatus>,
        /// List of conditions for the resource
        pub conditions: Option<Vec<Condition>>,
    }

    /// Status of a Deployment resource
    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
    pub struct DeploymentStatus {
        pub name: String,
        pub ready_replicas: u32,
        pub desired_replicas: u32,
        pub available: bool,
        pub reason: Option<String>,
    }

    /// Status of a Service resource
    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
    pub struct ServiceStatus {
        pub name: String,
        #[serde(rename = "type")]
        pub type_: Option<String>,
        pub cluster_ip: Option<String>,
        pub ready: bool,
    }

    /// Status of a Storage resource (PV or PVC)
    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
    pub struct StorageStatus {
        pub name: String,
        pub kind: String,
        pub bound: bool,
    }

    /// The general settings
    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
    pub struct GeneralSpec {
        /// The storage size in MB
        ///
        /// 0 means ephemeral
        pub storage: u32,
    }
}

/// Create or update the CustomResourceDefinition object
///
/// # Arguments
/// - `client` - The Kubernetes client
pub async fn crd_deploy(client: Client) -> Result<CustomResourceDefinition, Error> {
    let crd: CustomResourceDefinition = v1::pgopr::crd();
    trace!("{:#?}", crd);

    let crd_api: Api<CustomResourceDefinition> = Api::all(client.clone());
    let result: Result<CustomResourceDefinition, Error> =
        match crd_api.create(&PostParams::default(), &crd).await {
            Ok(crd) => {
                info!("Created CRD");
                Ok(crd)
            }
            Err(Error::Api(err)) if err.code == 409 => {
                let patch = Patch::Merge(&crd);
                let crd = crd_api
                    .patch("pgoprs.pgopr.io", &PatchParams::default(), &patch)
                    .await?;
                info!("Updated CRD");
                Ok(crd)
            }
            Err(err) => Err(err),
        };

    let establish = await_condition(crd_api, "pgoprs.pgopr.io", conditions::is_crd_established());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), establish).await;

    result
}

/// Delete the CustomResourceDefinition object
///
/// # Arguments:
/// - `client` - The Kubernetes client
///
/// Note: It is assumed the deployment exists for simplicity. Otherwise returns an Error.
pub async fn crd_undeploy(client: Client) -> Result<(), Error> {
    let api: Api<CustomResourceDefinition> = Api::all(client);
    match api
        .delete("pgoprs.pgopr.io", &DeleteParams::default())
        .await
    {
        Ok(_) => {
            info!("Deleted CRD");
        }

        Err(e) => return Err(e),
    }

    Ok(())
}

/// CRD: Generate
pub fn crd_generate() {
    let data = serde_yaml::to_string(&v1::pgopr::crd()).expect("Can't serialize pgopr-crd.yaml");
    fs::write("pgopr-crd.yaml", data).expect("Unable to write file: pgopr-crd.yaml");
}
