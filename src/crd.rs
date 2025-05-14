/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::CustomResource;
use kube::{
    Api, Client, Error,
    api::{DeleteParams, PostParams},
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
        // apiextensions = "v1",
        group = "pgopr.io",
        version = "v1",
        kind = "pgopr",
        plural = "pgoprs",
        derive = "PartialEq",
        namespaced
    )]
    pub struct PgOprSpec {
        /// General settings across all components
        pub storage: u32,
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

/// Create the CustomResoureDefinition object
///
/// # Arguments
/// - `client` - The Kubernetes client
///
/// Note: It is assumed the resource does not already exists for simplicity. Returns an `Error` if it does.
pub async fn crd_deploy(client: Client) -> Result<CustomResourceDefinition, Error> {
    let crd: CustomResourceDefinition = v1::pgopr::crd();
    trace!("{:#?}", crd);

    // Create the object
    let crd_api: Api<CustomResourceDefinition> = Api::all(client.clone());
    let result: Result<CustomResourceDefinition, Error> =
        crd_api.create(&PostParams::default(), &crd).await;

    let establish = await_condition(crd_api, "pgopr.io", conditions::is_crd_established());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), establish).await;

    if result.is_ok() {
        info!("Created CRD");
    }

    return result;
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

        Err(e) => return Err(e.into()),
    }

    Ok(())
}

/// CRD: Generate
pub fn crd_generate() {
    let data = serde_yaml::to_string(&v1::pgopr::crd()).expect("Can't serialize pgopr-crd.yaml");
    fs::write("pgopr-crd.yaml", data).expect("Unable to write file: pgopr-crd.yaml");
}
