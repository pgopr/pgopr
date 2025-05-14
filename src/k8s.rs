/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use kube::client::Client;
use log::error;
use std::process;

pub async fn k8s_client() -> Client {
    Client::try_default().await.unwrap_or_else(|_| {
        error!("Fatal error: Expected a valid KUBECONFIG environment variable.");
        process::exit(1);
    })
}
