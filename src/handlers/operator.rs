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
    Api,
    runtime::{Controller, watcher},
};
use log::{debug, error, info};
use std::sync::Arc;

/// Initializes and starts the Kubernetes controller loop for pgopr resources.
pub async fn run_operator() {
    super::print_header();

    let client = k8s::k8s_client().await;
    let target_ns =
        std::env::var("PGOPR_TARGET_NAMESPACE").unwrap_or_else(|_| "default".to_string());

    info!("watching namespace: {}", target_ns);

    let crd_api: Api<pgopr> = if target_ns == "*" {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), &target_ns)
    };

    let context = Arc::new(ContextData::new(client.clone()));

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
