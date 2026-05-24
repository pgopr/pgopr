/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::ConfigMap;
use kube::ResourceExt;
use kube::api::ObjectMeta;

use crate::Error;
use crate::crd::v1::pgopr;
use crate::manager::{self as k8s_manager, ResourceManager};

const CONFIG_FILE_NAME: &str = "postgresql.conf";

pub struct ConfigResult {
    pub name: String,
    pub hash: String,
}

/// Ensures an immutable ConfigMap exists for the given configuration.
pub async fn sync_config(
    manager: &ResourceManager,
    owner: &pgopr,
    config: &BTreeMap<String, String>,
) -> Result<ConfigResult, crate::Error> {
    validate_config_keys(config)?;

    let hash = config_hash(config);

    let cluster_name = owner.name_any();
    let cm_name = format!("{}-config-{}", cluster_name, hash);

    let mut config_file = String::new();
    for (key, value) in config {
        config_file.push_str(&format!("{} = '{}'\n", key, escape_config_value(value)));
    }

    let mut data = BTreeMap::new();
    data.insert(CONFIG_FILE_NAME.to_string(), config_file);

    let namespace = owner
        .namespace()
        .unwrap_or_else(|| k8s_manager::DEFAULT_NAMESPACE.to_string());

    let cm = ConfigMap {
        metadata: ObjectMeta {
            name: Some(cm_name.clone()),
            namespace: Some(namespace.clone()),
            ..Default::default()
        },
        data: Some(data),
        immutable: Some(true),
        ..Default::default()
    };

    manager.sync(owner, cm).await?;

    Ok(ConfigResult {
        name: cm_name,
        hash,
    })
}

fn validate_config_keys(config: &BTreeMap<String, String>) -> Result<(), crate::Error> {
    for key in config.keys() {
        if key.is_empty() {
            return Err(Error::UserInputError(format!(
                "Invalid PostgreSQL config key: {}",
                key
            )));
        }
    }

    Ok(())
}

fn config_hash(config: &BTreeMap<String, String>) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for (key, value) in config {
        hash = fnv1a(hash, key.as_bytes());
        hash = fnv1a(hash, b"\0");
        hash = fnv1a(hash, value.as_bytes());
        hash = fnv1a(hash, b"\0");
    }
    format!("{hash:016x}")[..8].to_string()
}

fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn escape_config_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "''")
}
