/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use k8s_openapi::api::core::v1::{Service, ServicePort, ServiceSpec};
use kube::{
    Api, Client, Error,
    api::{DeleteParams, ObjectMeta, PostParams},
};
use log::{info, trace};
use std::collections::BTreeMap;
use std::fs;

/// Creates a service
///
/// # Arguments
/// - `client` - The Kubernetes client
/// - `name` - The name
/// - `namespace` - The namespace
///
/// Note: It is assumed the service does not already exists for simplicity. Returns an `Error` if it does.
pub async fn service_deploy(client: Client, name: &str, namespace: &str) -> Result<Service, Error> {
    // Definition of the service
    let s: Service = service_create(name, namespace);
    trace!("{:#?}", s);

    // Create the deployment defined above
    let s_api: Api<Service> = Api::namespaced(client, namespace);
    let result: Result<Service, Error> = s_api.create(&PostParams::default(), &s).await;

    if result.is_ok() {
        info!("Created Service");
    }

    return result;
}

/// Deletes a service
///
/// # Arguments:
/// - `client` - The Kubernetes client
/// - `name` - The name of the service
/// - `namespace` - The namespace
///
/// Note: It is assumed the service exists for simplicity. Otherwise returns an Error.
pub async fn service_undeploy(client: Client, name: &str, namespace: &str) -> Result<(), Error> {
    let api: Api<Service> = Api::namespaced(client, namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => {
            info!("Deleted Service");
        }

        Err(e) => return Err(e.into()),
    }

    Ok(())
}

/// Service: Generate
pub fn service_generate() {
    let data = serde_yaml::to_string(&service_create("postgresql", "default"))
        .expect("Can't serialize pgopr-service.yaml");
    fs::write("pgopr-service.yaml", data).expect("Unable to write file: pgopr-service.yaml");
}

fn service_create(name: &str, namespace: &str) -> Service {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), "postgresql".to_owned());

    let s: Service = Service {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(namespace.to_owned()),
            labels: Some(labels.clone()),
            ..ObjectMeta::default()
        },
        spec: Some(ServiceSpec {
            type_: Some("NodePort".to_owned()),
            ports: Some(vec![ServicePort {
                port: 5432,
                ..ServicePort::default()
            }]),
            selector: Some(labels.clone()),
            ..ServiceSpec::default()
        }),
        ..Service::default()
    };

    s
}
