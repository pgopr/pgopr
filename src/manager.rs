/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use crate::Error;
use crate::crd::v1::pgopr;
use kube::core::{ClusterResourceScope, NamespaceResourceScope};
use kube::{
    Api, Client, Resource,
    api::{DeleteParams, Patch, PatchParams, ResourceExt},
};
use log::info;
use serde::{Serialize, de::DeserializeOwned};
use std::fmt::Debug;

/// Global Constants for the Operator
pub const MANAGER_NAME: &str = "pgopr-manager";
pub const DEFAULT_NAMESPACE: &str = "default";

/// ResourceManager handles Kubernetes API writes for managed resources.
pub struct ResourceManager {
    client: Client,
}

impl ResourceManager {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub fn get_client(&self) -> Client {
        self.client.clone()
    }

    /// Syncs a namespaced Kubernetes resource using Server-Side Apply.
    ///
    /// # Arguments
    /// - `owner` - The PgOpr resource that owns the Kubernetes resource.
    /// - `resource` - The Kubernetes resource to sync.
    pub async fn sync<K>(&self, owner: &pgopr, mut resource: K) -> Result<K, Error>
    where
        K: Resource<Scope = NamespaceResourceScope> + Clone + Debug + Serialize + DeserializeOwned,
        K::DynamicType: Default,
    {
        let name = resource.name_any();
        let namespace = resource
            .namespace()
            .unwrap_or_else(|| DEFAULT_NAMESPACE.to_string());

        if let Some(owner_ref) = owner.controller_owner_ref(&()) {
            resource.meta_mut().owner_references = Some(vec![owner_ref]);
        }

        let api: Api<K> = Api::namespaced(self.client.clone(), &namespace);
        let params = PatchParams::apply(MANAGER_NAME);

        info!(
            "Syncing {} resource: {}/{}",
            K::kind(&Default::default()),
            namespace,
            name
        );

        api.patch(&name, &params, &Patch::Apply(&resource))
            .await
            .map_err(Error::from)
    }

    /// Syncs a cluster-scoped resource using Server-Side Apply.
    ///
    /// # Arguments
    /// - `resource` - The Kubernetes resource to sync.
    pub async fn sync_cluster<K>(&self, resource: K) -> Result<K, Error>
    where
        K: Resource<Scope = ClusterResourceScope> + Clone + Debug + Serialize + DeserializeOwned,
        K::DynamicType: Default,
    {
        let name = resource.name_any();
        let api: Api<K> = Api::all(self.client.clone());
        let params = PatchParams::apply(MANAGER_NAME);

        info!(
            "Syncing {} resource: {}",
            K::kind(&Default::default()),
            name
        );

        api.patch(&name, &params, &Patch::Apply(&resource))
            .await
            .map_err(Error::from)
    }

    /// Deletes a namespaced resource, treating an already-deleted resource as success.
    ///
    /// # Arguments
    /// - `name` - Name of the Kubernetes resource to delete.
    /// - `namespace` - Namespace where the Kubernetes resource resides.
    pub async fn delete<K>(&self, name: &str, namespace: &str) -> Result<(), Error>
    where
        K: Resource<Scope = NamespaceResourceScope> + Clone + Debug + Serialize + DeserializeOwned,
        K::DynamicType: Default,
    {
        let api: Api<K> = Api::namespaced(self.client.clone(), namespace);
        match api.delete(name, &DeleteParams::default()).await {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(err)) if err.code == 404 => Ok(()),
            Err(err) => Err(Error::from(err)),
        }
    }

    /// Deletes a cluster-scoped resource, treating an already-deleted resource as success.
    ///
    /// # Arguments
    /// - `name` - Name of the Kubernetes resource to delete.
    pub async fn delete_cluster<K>(&self, name: &str) -> Result<(), Error>
    where
        K: Resource<Scope = ClusterResourceScope> + Clone + Debug + Serialize + DeserializeOwned,
        K::DynamicType: Default,
    {
        let api: Api<K> = Api::all(self.client.clone());
        match api.delete(name, &DeleteParams::default()).await {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(err)) if err.code == 404 => Ok(()),
            Err(err) => Err(Error::from(err)),
        }
    }
}
