/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use crate::{ContextData, k8s, on_error, pgopr, reconcile};
use futures::StreamExt;
use kube::{
    Api, Client,
    runtime::{Controller, watcher},
};
use log::{debug, error};
use std::sync::Arc;

/// Initializes and starts the Kubernetes controller loop for pgopr resources.
pub async fn run_operator() {
    super::print_header();

    let client: Client = k8s::k8s_client().await;
    let crd_api: Api<pgopr> = Api::all(client.clone());
    let context: Arc<ContextData> = Arc::new(ContextData::new(client.clone()));

    // Start the controller
    Controller::new(crd_api.clone(), watcher::Config::default())
        .run(reconcile, on_error, context)
        .for_each(|reconciliation_result| async move {
            match reconciliation_result {
                Ok(pgopr_resource) => {
                    debug!("Reconciliation successful. Resource: {:?}", pgopr_resource);
                }
                Err(reconciliation_err) => {
                    error!("Reconciliation error: {:?}", reconciliation_err)
                }
            }
        })
        .await;
}
