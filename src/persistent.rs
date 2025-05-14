/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use k8s_openapi::{
    api::core::v1::{
        HostPathVolumeSource, PersistentVolume, PersistentVolumeClaim, PersistentVolumeClaimSpec,
        PersistentVolumeSpec, VolumeResourceRequirements,
    },
    apimachinery::pkg::api::resource::Quantity,
};
use kube::{
    Api, Client, Error,
    api::{DeleteParams, ObjectMeta, PostParams},
};
use log::{info, trace};
use std::collections::BTreeMap;
use std::fs;

/// Creates a persistent volume
///
/// # Arguments
/// - `client` - The Kubernetes client
/// - `name` - The name
/// - `storage` - The storage size
///
/// Note: It is assumed the persistent volume does not already exists for simplicity. Returns an `Error` if it does.
pub async fn persistent_volume_deploy(
    client: Client,
    name: &str,
    storage: u32,
) -> Result<PersistentVolume, Error> {
    // Definition of the persistent volume
    let pv: PersistentVolume = pv_create(name, storage);
    trace!("pv: {:?}", pv);

    // Create the persistent volume defined above
    let pv_api: Api<PersistentVolume> = Api::all(client);
    let pp: PostParams = PostParams::default();
    match pv_api.create(&pp, &pv).await {
        Ok(o) => {
            info!("Created PersistentVolume");
            return Ok(o);
        }
        Err(e) => return Err(e.into()),
    }
}

/// Deletes an existing persistent volume
///
/// # Arguments:
/// - `client` - The Kubernetes client
/// - `name` - The name of the persistent volume
///
/// Note: It is assumed the persistence volume exists for simplicity. Otherwise returns an Error.
pub async fn persistent_volume_undeploy(client: Client, name: &str) -> Result<(), Error> {
    let api: Api<PersistentVolume> = Api::all(client);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => {
            info!("Deleted PersistentVolume");
        }

        Err(e) => return Err(e.into()),
    }
    Ok(())
}

/// Creates a persistent volume claim
///
/// # Arguments
/// - `client` - The Kubernetes client
/// - `name` - The name
/// - `namespace` - The namespace
/// - `storage` - The storage size
///
/// Note: It is assumed the persistent volume claim does not already exists for simplicity. Returns an `Error` if it does.
pub async fn persistent_volume_claim_deploy(
    client: Client,
    name: &str,
    namespace: &str,
    storage: u32,
) -> Result<PersistentVolumeClaim, Error> {
    // Definition of the persistent volume claim
    let pvc: PersistentVolumeClaim = pvc_create(name, namespace, storage);
    trace!("pvc: {:?}", pvc);

    // Create the persistent volume claim defined above
    let pvc_api: Api<PersistentVolumeClaim> = Api::namespaced(client, namespace);
    let pp: PostParams = PostParams::default();
    match pvc_api.create(&pp, &pvc).await {
        Ok(o) => {
            info!("Created PersistentVolumeClaim");
            return Ok(o);
        }
        Err(e) => return Err(e.into()),
    }
}

/// Deletes an existing persistent volume claim
///
/// # Arguments:
/// - `client` - The Kubernetes client
/// - `name` - The name of the persistent volume claim
/// - `namespace` - The namespace
///
/// Note: It is assumed the persistence volume claim exists for simplicity. Otherwise returns an Error.
pub async fn persistent_volume_claim_undeploy(
    client: Client,
    name: &str,
    namespace: &str,
) -> Result<(), Error> {
    let api: Api<PersistentVolumeClaim> = Api::namespaced(client, namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => {
            info!("Deleted PersistentVolumeClaim");
        }

        Err(e) => return Err(e.into()),
    }
    Ok(())
}

/// Persistent: Generate
pub fn persistent_generate() {
    let pv = serde_yaml::to_string(&pv_create("postgresql-pv-volume", 5u32))
        .expect("Can't serialize pgopr-pv.yaml");
    fs::write("pgopr-pv.yaml", pv).expect("Unable to write file: pgopr-pv.yaml");

    let pvc = serde_yaml::to_string(&pvc_create("postgresql-pv-claim", "default", 5u32))
        .expect("Can't serialize pgopr-pvc.yaml");
    fs::write("pgopr-pvc.yaml", pvc).expect("Unable to write file: pgopr-pvc.yaml");
}

fn pv_create(name: &str, storage: u32) -> PersistentVolume {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), "postgresql".to_owned());
    labels.insert("type".to_owned(), "local".to_owned());

    let mut size: String = storage.to_string().to_owned();
    size.push_str(&"Gi".to_owned());

    let mut cap: BTreeMap<String, Quantity> = BTreeMap::new();
    cap.insert("storage".to_owned(), Quantity(size));

    // Definition of the deployment
    let pv: PersistentVolume = PersistentVolume {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            labels: Some(labels.clone()),
            ..ObjectMeta::default()
        },
        spec: Some(PersistentVolumeSpec {
            storage_class_name: Some("manual".to_owned()),
            capacity: Some(cap.clone()),
            access_modes: Some(vec!["ReadWriteMany".to_owned()]),
            host_path: Some(HostPathVolumeSource {
                path: "/tmp/kind".to_owned(),
                ..HostPathVolumeSource::default()
            }),
            ..PersistentVolumeSpec::default()
        }),
        ..PersistentVolume::default()
    };

    pv
}

fn pvc_create(name: &str, namespace: &str, storage: u32) -> PersistentVolumeClaim {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), "postgresql".to_owned());

    let mut size: String = storage.to_string().to_owned();
    size.push_str(&"Gi".to_owned());

    let mut cap: BTreeMap<String, Quantity> = BTreeMap::new();
    cap.insert("storage".to_owned(), Quantity(size));

    // Definition of the persistent volume claim
    let pvc: PersistentVolumeClaim = PersistentVolumeClaim {
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
    };

    pvc
}
