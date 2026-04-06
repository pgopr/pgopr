/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use crate::{crd, k8s, persistent, pgexporter, primary, services};
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

    let _pv = persistent::persistent_volume_deploy(
        client.clone(),
        "postgresql-pv-volume",
        5u32,
        "postgresql",
        "/tmp/kind",
    )
    .await;
    let _pvc = persistent::persistent_volume_claim_deploy(
        client.clone(),
        "postgresql-pv-claim",
        &namespace,
        5u32,
        "postgresql",
    )
    .await;
    let _d = primary::primary_deploy(client.clone(), "postgresql", &namespace).await;
    let _s = services::service_deploy(client.clone(), "postgresql", &namespace, 5432).await;
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

/// Provisions the replica database components (PV, PVC, Deployment, Service).
pub async fn handle_provision_replica() {
    super::print_header();
    debug!("replica");
    let client: Client = k8s::k8s_client().await;
    let namespace = "default".to_owned();

    let _pv = persistent::persistent_volume_deploy(
        client.clone(),
        "postgresql-replica-pv-volume",
        5u32,
        "postgresql-replica",
        "/tmp/kind-replica",
    )
    .await;
    let _pvc = persistent::persistent_volume_claim_deploy(
        client.clone(),
        "postgresql-replica-pv-claim",
        &namespace,
        5u32,
        "postgresql-replica",
    )
    .await;
    let _d = crate::replica::replica_deploy(
        client.clone(),
        "postgresql-replica",
        "postgresql",
        &namespace,
        "replica1",
    )
    .await;
    let _s = services::service_deploy(client.clone(), "postgresql-replica", &namespace, 5432).await;
}

/// Provisions pgexporter (Deployment, Service).
pub async fn handle_provision_pgexporter() {
    super::print_header();
    debug!("pgexporter");
    let client: Client = k8s::k8s_client().await;
    let namespace = "default".to_owned();

    let _d = pgexporter::pgexporter_deploy(
        client.clone(),
        "postgresql-pgexporter",
        "postgresql",
        &namespace,
    )
    .await;
    let _s =
        services::service_deploy(client.clone(), "postgresql-pgexporter", &namespace, 5002).await;
}

/// Removes pgexporter components.
pub async fn handle_retire_pgexporter() {
    super::print_header();
    debug!("pgexporter");
    let client: Client = k8s::k8s_client().await;
    let namespace = "default".to_owned();

    let _s = services::service_undeploy(client.clone(), "postgresql-pgexporter", &namespace).await;
    let _d =
        pgexporter::pgexporter_undeploy(client.clone(), "postgresql-pgexporter", &namespace).await;
}

/// Removes the replica database components.
pub async fn handle_retire_replica() {
    super::print_header();
    debug!("replica");
    let client: Client = k8s::k8s_client().await;
    let namespace = "default".to_owned();

    let _s = services::service_undeploy(client.clone(), "postgresql-replica", &namespace).await;
    let _d =
        crate::replica::replica_undeploy(client.clone(), "postgresql-replica", &namespace).await;
    let _pvc = persistent::persistent_volume_claim_undeploy(
        client.clone(),
        "postgresql-replica-pv-claim",
        &namespace,
    )
    .await;
    let _pv =
        persistent::persistent_volume_undeploy(client.clone(), "postgresql-replica-pv-volume")
            .await;
}
