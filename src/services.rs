/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use k8s_openapi::api::core::v1::{Service, ServicePort, ServiceSpec};
use kube::api::ObjectMeta;
use std::collections::BTreeMap;

/// Builds a service object
///
/// # Arguments
/// - `name` - The name
/// - `namespace` - The namespace
pub fn build(name: &str, namespace: &str, port: i32) -> Service {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), name.to_owned());

    Service {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(namespace.to_owned()),
            labels: Some(labels.clone()),
            ..ObjectMeta::default()
        },
        spec: Some(ServiceSpec {
            type_: Some("NodePort".to_owned()),
            ports: Some(vec![ServicePort {
                port,
                ..ServicePort::default()
            }]),
            selector: Some(labels.clone()),
            ..ServiceSpec::default()
        }),
        ..Service::default()
    }
}
