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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_sets_metadata_and_selector() {
        let svc = build("postgresql", "default", 5432);

        assert_eq!(svc.metadata.name.as_deref(), Some("postgresql"));
        assert_eq!(svc.metadata.namespace.as_deref(), Some("default"));

        let labels = svc.metadata.labels.unwrap();
        assert_eq!(labels.get("app").unwrap(), "postgresql");

        let spec = svc.spec.unwrap();
        assert_eq!(spec.selector.unwrap(), labels);
    }

    #[test]
    fn test_build_sets_port_and_type() {
        let svc = build("postgresql", "default", 5432);
        let spec = svc.spec.unwrap();

        assert_eq!(spec.type_.as_deref(), Some("NodePort"));

        let ports = spec.ports.unwrap();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 5432);
    }
}
