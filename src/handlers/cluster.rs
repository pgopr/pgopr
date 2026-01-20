/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use crate::{crd, k8s, persistent, primary, services};
use kube::Client;
use log::debug;

/// Orchestrates the installation of the operator and its CRDs.
pub async fn handle_install() {
    super::print_header();
    let client: Client = k8s::k8s_client().await;
    let _ = crd::crd_deploy(client).await;
}

/// Orchestrates the uninstallation of the operator and its CRDs.
pub async fn handle_uninstall() {
    super::print_header();
    let client: Client = k8s::k8s_client().await;
    let _ = crd::crd_undeploy(client).await;
}

/// Provisions the primary database components (PV, PVC, Deployment, Service).
pub async fn handle_provision_primary() {
    super::print_header();
    debug!("primary");
    let client: Client = k8s::k8s_client().await;
    let namespace = "default".to_owned();

    let _pv =
        persistent::persistent_volume_deploy(client.clone(), "postgresql-pv-volume", 5u32).await;
    let _pvc = persistent::persistent_volume_claim_deploy(
        client.clone(),
        "postgresql-pv-claim",
        &namespace,
        5u32,
    )
    .await;
    let _d = primary::primary_deploy(client.clone(), "postgresql", &namespace).await;
    let _s = services::service_deploy(client.clone(), "postgresql", &namespace).await;
}

/// Removes the primary database components.
pub async fn handle_retire_primary() {
    super::print_header();
    debug!("primary");
    let client: Client = k8s::k8s_client().await;
    let namespace = "default".to_owned();

    let _s = services::service_undeploy(client.clone(), "postgresql", &namespace).await;
    let _d = primary::primary_undeploy(client.clone(), "postgresql", &namespace).await;
    let _pvc = persistent::persistent_volume_claim_undeploy(
        client.clone(),
        "postgresql-pv-claim",
        &namespace,
    )
    .await;
    let _pv = persistent::persistent_volume_undeploy(client.clone(), "postgresql-pv-volume").await;
}
