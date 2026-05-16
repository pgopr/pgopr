/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use crate::manager::LABEL_CLUSTER;
use k8s_openapi::api::core::v1::{HostPathVolumeSource, PersistentVolume, PersistentVolumeSpec};
use k8s_openapi::{
    api::core::v1::{PersistentVolumeClaim, PersistentVolumeClaimSpec, VolumeResourceRequirements},
    apimachinery::pkg::api::resource::Quantity,
};
use kube::api::ObjectMeta;
use std::collections::BTreeMap;

/// Builds a persistent volume claim object
///
/// # Arguments
/// - `name` - The name
/// - `namespace` - The namespace
/// - `storage` - The storage size
pub fn build_pvc(
    name: &str,
    namespace: &str,
    storage: u32,
    label_app: &str,
) -> PersistentVolumeClaim {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), label_app.to_owned());

    let mut size: String = storage.to_string().to_owned();
    size.push_str("Gi");

    let mut cap: BTreeMap<String, Quantity> = BTreeMap::new();
    cap.insert("storage".to_owned(), Quantity(size));

    // Definition of the persistent volume claim
    PersistentVolumeClaim {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(namespace.to_owned()),
            labels: Some(labels.clone()),
            ..ObjectMeta::default()
        },
        spec: Some(PersistentVolumeClaimSpec {
            storage_class_name: Some("manual".to_owned()),
            access_modes: Some(vec!["ReadWriteMany".to_owned()]),
            resources: Some(VolumeResourceRequirements {
                requests: Some(cap.clone()),
                ..VolumeResourceRequirements::default()
            }),
            ..PersistentVolumeClaimSpec::default()
        }),
        ..PersistentVolumeClaim::default()
    }
}

/// Builds a persistent volume object for the manual local storage path.
pub fn build_pv(
    name: &str,
    storage: u32,
    label_app: &str,
    host_path: &str,
    cluster_name: &str,
) -> PersistentVolume {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), label_app.to_owned());
    labels.insert("type".to_owned(), "local".to_owned());
    labels.insert(LABEL_CLUSTER.to_string(), cluster_name.to_string());

    let mut size: String = storage.to_string().to_owned();
    size.push_str("Gi");

    let mut cap: BTreeMap<String, Quantity> = BTreeMap::new();
    cap.insert("storage".to_owned(), Quantity(size));

    PersistentVolume {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            labels: Some(labels),
            ..ObjectMeta::default()
        },
        spec: Some(PersistentVolumeSpec {
            storage_class_name: Some("manual".to_owned()),
            capacity: Some(cap),
            access_modes: Some(vec!["ReadWriteMany".to_owned()]),
            host_path: Some(HostPathVolumeSource {
                path: host_path.to_owned(),
                ..HostPathVolumeSource::default()
            }),
            ..PersistentVolumeSpec::default()
        }),
        ..PersistentVolume::default()
    }
}
