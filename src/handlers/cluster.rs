/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use crate::crd::v1::{PgExporterSpec, PgMonetaSpec, PgOprSpec, pgopr};
use crate::k8s;
use crate::local_config::{LocalConfig, load_config};
use kube::{
    Api, Client,
    api::{DeleteParams, Patch, PatchParams, PostParams},
};
use log::error;

/// Orchestrates the installation of the operator and its CRDs.
pub async fn handle_install() {
    super::print_header();
    let client: Client = k8s::k8s_client().await;
    let _ = crate::crd::crd_deploy(client).await;
}

/// Orchestrates the uninstallation of the operator and its CRDs.
pub async fn handle_uninstall() {
    super::print_header();
    let client: Client = k8s::k8s_client().await;
    let _ = crate::crd::crd_undeploy(client).await;
}

/// Creates the PgOpr resource.
///
/// # Arguments
/// - `client` - Kubernetes client to create the PgOpr resource with.
/// - `name` - Name of the PgOpr resource to create.
/// - `namespace` - Namespace where the PgOpr resource resides.
/// - `replicas` - Desired number of replicas.
/// - `storage` - Desired storage size.
/// - `pgmoneta` - Optional pgmoneta backup specification.
/// - `pgexporter` - Optional pgexporter metrics specification.
async fn create_cluster(
    client: Client,
    name: &str,
    namespace: &str,
    replicas: u32,
    storage: u32,
    pgmoneta: Option<PgMonetaSpec>,
    pgexporter: Option<PgExporterSpec>,
) -> Result<pgopr, crate::Error> {
    let api: Api<pgopr> = Api::namespaced(client, namespace);
    let mut cluster = pgopr::new(
        name,
        PgOprSpec {
            version: None,
            storage,
            replicas: Some(replicas),
            resources: None,
            config: None,
            pgmoneta,
            pgexporter,
        },
    );
    cluster.metadata.namespace = Some(namespace.to_string());

    api.create(&PostParams::default(), &cluster)
        .await
        .map_err(crate::Error::from)
}

/// Updates the replica count on an existing PgOpr resource.
///
/// # Arguments
/// - `client` - Kubernetes client to modify the PgOpr resource with.
/// - `name` - Name of the PgOpr resource to modify.
/// - `namespace` - Namespace where the PgOpr resource resides.
/// - `replicas` - Desired number of replicas.
async fn patch_replicas(
    client: Client,
    name: &str,
    namespace: &str,
    replicas: u32,
) -> Result<pgopr, crate::Error> {
    let api: Api<pgopr> = Api::namespaced(client, namespace);
    let patch = serde_json::json!({
        "spec": {
            "replicas": replicas
        }
    });

    api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
        .await
        .map_err(crate::Error::from)
}

/// Gets the PgOpr resource.
///
/// # Arguments
/// - `client` - Kubernetes client to get the PgOpr resource with.
/// - `name` - Name of the PgOpr resource to get.
/// - `namespace` - Namespace where the PgOpr resource resides.
async fn get_cluster(client: Client, name: &str, namespace: &str) -> Result<pgopr, crate::Error> {
    let api: Api<pgopr> = Api::namespaced(client, namespace);
    api.get(name).await.map_err(crate::Error::from)
}

/// Provisions the primary database components through a PgOpr resource.
pub async fn handle_provision_primary() {
    super::print_header();
    let local_cfg = load_config();
    let client: Client = k8s::k8s_client().await;
    match get_cluster(
        client.clone(),
        &local_cfg.cluster_name,
        &local_cfg.namespace,
    )
    .await
    {
        Ok(_) => {}
        Err(crate::Error::KubeError { source }) => match source {
            kube::Error::Api(err) if err.code == 404 => {
                if let Err(err) = create_cluster(
                    client,
                    &local_cfg.cluster_name,
                    &local_cfg.namespace,
                    0,
                    local_cfg.default_storage,
                    None,
                    None,
                )
                .await
                {
                    error!("Unable to create PgOpr resource: {:?}", err);
                }
            }
            err => error!("Unable to get PgOpr resource: {:?}", err),
        },
        Err(err) => error!("Unable to get PgOpr resource: {:?}", err),
    }
}

async fn ensure_component(
    client: Client,
    local_cfg: &LocalConfig,
    component: &str,
    patch: serde_json::Value,
    pgmoneta: Option<PgMonetaSpec>,
    pgexporter: Option<PgExporterSpec>,
) {
    match get_cluster(
        client.clone(),
        &local_cfg.cluster_name,
        &local_cfg.namespace,
    )
    .await
    {
        Ok(_) => {
            let api: Api<pgopr> = Api::namespaced(client, &local_cfg.namespace);
            if let Err(err) = api
                .patch(
                    &local_cfg.cluster_name,
                    &PatchParams::default(),
                    &Patch::Merge(&patch),
                )
                .await
            {
                error!("Unable to patch PgOpr {}: {:?}", component, err);
            }
        }
        Err(crate::Error::KubeError { source }) => match source {
            kube::Error::Api(err) if err.code == 404 => {
                if let Err(err) = create_cluster(
                    client,
                    &local_cfg.cluster_name,
                    &local_cfg.namespace,
                    0,
                    local_cfg.default_storage,
                    pgmoneta,
                    pgexporter,
                )
                .await
                {
                    error!("Unable to create PgOpr resource: {:?}", err);
                }
            }
            err => error!("Unable to get PgOpr resource: {:?}", err),
        },
        Err(err) => error!("Unable to get PgOpr resource: {:?}", err),
    }
}

async fn provision_pgmoneta(client: Client, local_cfg: &LocalConfig) {
    ensure_component(
        client,
        local_cfg,
        "pgmoneta",
        serde_json::json!({
            "spec": {
                "pgmoneta": {
                    "storage": local_cfg.default_pgmoneta_storage
                }
            }
        }),
        Some(PgMonetaSpec {
            storage: Some(local_cfg.default_pgmoneta_storage),
        }),
        None,
    )
    .await;
}

async fn provision_pgexporter(client: Client, local_cfg: &LocalConfig) {
    ensure_component(
        client,
        local_cfg,
        "pgexporter",
        serde_json::json!({
            "spec": {
                "pgexporter": {}
            }
        }),
        None,
        Some(PgExporterSpec {
            resources: None,
            monitoring: None,
        }),
    )
    .await;
}

/// Provisions pgmoneta through the PgOpr resource.
pub async fn handle_provision_pgmoneta() {
    super::print_header();
    let local_cfg = load_config();
    let client: Client = k8s::k8s_client().await;
    provision_pgmoneta(client, &local_cfg).await;
}

/// Provisions pgexporter through the PgOpr resource.
pub async fn handle_provision_pgexporter() {
    super::print_header();
    let local_cfg = load_config();
    let client: Client = k8s::k8s_client().await;
    provision_pgexporter(client, &local_cfg).await;
}

/// Retires pgexporter through the PgOpr resource.
pub async fn handle_retire_pgexporter() {
    super::print_header();
    let local_cfg = load_config();
    let client: Client = k8s::k8s_client().await;
    let api: Api<pgopr> = Api::namespaced(client, &local_cfg.namespace);
    let patch = serde_json::json!({
        "spec": {
            "pgexporter": null
        }
    });
    if let Err(err) = api
        .patch(
            &local_cfg.cluster_name,
            &PatchParams::default(),
            &Patch::Merge(&patch),
        )
        .await
    {
        error!("Unable to patch PgOpr pgexporter: {:?}", err);
    }
}

/// Retires pgmoneta through the PgOpr resource.
pub async fn handle_retire_pgmoneta() {
    super::print_header();
    let local_cfg = load_config();
    let client: Client = k8s::k8s_client().await;
    let api: Api<pgopr> = Api::namespaced(client, &local_cfg.namespace);
    let patch = serde_json::json!({
        "spec": {
            "pgmoneta": null
        }
    });
    if let Err(err) = api
        .patch(
            &local_cfg.cluster_name,
            &PatchParams::default(),
            &Patch::Merge(&patch),
        )
        .await
    {
        error!("Unable to patch PgOpr pgmoneta: {:?}", err);
    }
}

/// Provisions pgexporter monitoring (Grafana + Prometheus) through the PgOpr resource.
pub async fn handle_provision_grafana() {
    super::print_header();
    let local_cfg = load_config();
    let client: Client = k8s::k8s_client().await;
    match get_cluster(
        client.clone(),
        &local_cfg.cluster_name,
        &local_cfg.namespace,
    )
    .await
    {
        Ok(_) => {
            let api: Api<pgopr> = Api::namespaced(client, &local_cfg.namespace);
            let patch = serde_json::json!({
                "spec": {
                    "pgexporter": {
                        "monitoring": {}
                    }
                }
            });
            if let Err(err) = api
                .patch(
                    &local_cfg.cluster_name,
                    &PatchParams::default(),
                    &Patch::Merge(&patch),
                )
                .await
            {
                error!("Unable to patch PgOpr pgexporter monitoring: {:?}", err);
            }
        }
        Err(crate::Error::KubeError { source }) => match source {
            kube::Error::Api(err) if err.code == 404 => {
                error!(
                    "PgOpr resource does not exist. Create the cluster first with `pgopr provision primary`"
                );
            }
            err => error!("Unable to get PgOpr resource: {:?}", err),
        },
        Err(err) => error!("Unable to get PgOpr resource: {:?}", err),
    }
}

/// Retires pgexporter monitoring (Grafana + Prometheus) through the PgOpr resource.
pub async fn handle_retire_grafana() {
    super::print_header();
    let local_cfg = load_config();
    let client: Client = k8s::k8s_client().await;
    let api: Api<pgopr> = Api::namespaced(client, &local_cfg.namespace);
    let patch = serde_json::json!({
        "spec": {
            "pgexporter": {
                "monitoring": null
            }
        }
    });
    if let Err(err) = api
        .patch(
            &local_cfg.cluster_name,
            &PatchParams::default(),
            &Patch::Merge(&patch),
        )
        .await
    {
        error!("Unable to patch PgOpr pgexporter monitoring: {:?}", err);
    }
}

/// Removes the primary database components through the PgOpr resource.
pub async fn handle_retire_primary() {
    super::print_header();
    let local_cfg = load_config();
    let client: Client = k8s::k8s_client().await;
    let api: Api<pgopr> = Api::namespaced(client, &local_cfg.namespace);

    match api
        .delete(&local_cfg.cluster_name, &DeleteParams::default())
        .await
    {
        Ok(_) => {}
        Err(kube::Error::Api(err)) if err.code == 404 => {}
        Err(err) => {
            error!("Unable to delete PgOpr resource: {:?}", err);
        }
    }
}

/// Provisions the replica database components through a PgOpr resource.
pub async fn handle_provision_replica() {
    super::print_header();
    let local_cfg = load_config();
    let client: Client = k8s::k8s_client().await;
    match get_cluster(
        client.clone(),
        &local_cfg.cluster_name,
        &local_cfg.namespace,
    )
    .await
    {
        Ok(current) => {
            let replicas = current.spec.replicas.unwrap_or(0) + 1;
            if let Err(err) = patch_replicas(
                client,
                &local_cfg.cluster_name,
                &local_cfg.namespace,
                replicas,
            )
            .await
            {
                error!("Unable to patch PgOpr replicas: {:?}", err);
            }
        }
        Err(crate::Error::KubeError { source }) => match source {
            kube::Error::Api(err) if err.code == 404 => {
                if let Err(err) = create_cluster(
                    client,
                    &local_cfg.cluster_name,
                    &local_cfg.namespace,
                    1,
                    local_cfg.default_storage,
                    None,
                    None,
                )
                .await
                {
                    error!("Unable to create PgOpr resource: {:?}", err);
                }
            }
            err => error!("Unable to get PgOpr resource: {:?}", err),
        },
        Err(err) => error!("Unable to get PgOpr resource: {:?}", err),
    }
}

/// Removes the replica database components through a PgOpr resource.
pub async fn handle_retire_replica() {
    super::print_header();
    let local_cfg = load_config();
    let client: Client = k8s::k8s_client().await;
    match get_cluster(
        client.clone(),
        &local_cfg.cluster_name,
        &local_cfg.namespace,
    )
    .await
    {
        Ok(current) => {
            let replicas = current.spec.replicas.unwrap_or(0).saturating_sub(1);
            if let Err(err) = patch_replicas(
                client,
                &local_cfg.cluster_name,
                &local_cfg.namespace,
                replicas,
            )
            .await
            {
                error!("Unable to patch PgOpr replicas: {:?}", err);
            }
        }
        Err(err) => error!("Unable to get PgOpr resource: {:?}", err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn read_http_request(stream: &mut tokio::net::TcpStream) -> (String, String) {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let header_end;
        loop {
            let n = stream.read(&mut tmp).await.expect("read headers");
            assert!(n > 0, "unexpected EOF reading request");
            buf.extend_from_slice(&tmp[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                header_end = pos + 4;
                break;
            }
        }
        let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let content_len = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        while buf.len() < header_end + content_len {
            let n = stream.read(&mut tmp).await.expect("read body");
            assert!(n > 0, "unexpected EOF reading body");
            buf.extend_from_slice(&tmp[..n]);
        }
        let body = String::from_utf8_lossy(&buf[header_end..header_end + content_len]).to_string();
        (headers.lines().next().unwrap_or("").to_string(), body)
    }

    async fn write_json_response(stream: &mut tokio::net::TcpStream, status: &str, body: &str) {
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("write response");
    }

    async fn mock_cluster_server(
        first_get_status: &str,
        first_get_body: String,
        second_status: &str,
        second_body: String,
    ) -> (
        Client,
        tokio::task::JoinHandle<Vec<(String, serde_json::Value)>>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let cfg = kube::Config::new(format!("http://{addr}").parse().expect("valid uri"));
        let client = Client::try_from(cfg).expect("kube client");
        let first_status = first_get_status.to_string();
        let second_status = second_status.to_string();

        let handle = tokio::spawn(async move {
            let mut recorded = Vec::new();
            for (status, body) in [(first_status, first_get_body), (second_status, second_body)] {
                let (mut stream, _) = listener.accept().await.expect("accept conn");
                let (req_line, req_body) = read_http_request(&mut stream).await;
                let json_body = if req_body.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::from_str(&req_body).expect("json body")
                };
                recorded.push((req_line, json_body));
                write_json_response(&mut stream, &status, &body).await;
            }
            recorded
        });

        (client, handle)
    }

    fn not_found_status_json() -> String {
        serde_json::json!({
            "kind": "Status",
            "apiVersion": "v1",
            "status": "Failure",
            "message": "pgoprs.pgopr.io \"postgresql\" not found",
            "reason": "NotFound",
            "code": 404
        })
        .to_string()
    }

    fn sample_cluster_json() -> String {
        let mut cluster = pgopr::new(
            "postgresql",
            PgOprSpec {
                version: None,
                storage: 5,
                replicas: Some(0),
                resources: None,
                config: None,
                pgmoneta: None,
                pgexporter: None,
            },
        );
        cluster.metadata.namespace = Some("default".to_string());
        serde_json::to_string(&cluster).expect("serialize cluster")
    }

    #[tokio::test]
    async fn provision_pgmoneta_creates_cluster_with_pgmoneta_on_404() {
        let (client, server) = mock_cluster_server(
            "404 Not Found",
            not_found_status_json(),
            "201 Created",
            sample_cluster_json(),
        )
        .await;
        let cfg = LocalConfig {
            default_storage: 8,
            default_pgmoneta_storage: 25,
            ..LocalConfig::default()
        };

        provision_pgmoneta(client, &cfg).await;

        let calls = server.await.expect("server join");
        assert!(calls[0].0.starts_with("GET "));
        assert!(calls[1].0.starts_with("POST "));
        assert_eq!(calls[1].1["spec"]["storage"], 8);
        assert_eq!(calls[1].1["spec"]["pgmoneta"]["storage"], 25);
        assert!(calls[1].1["spec"]["pgexporter"].is_null());
    }

    #[tokio::test]
    async fn provision_pgexporter_creates_cluster_with_pgexporter_on_404() {
        let (client, server) = mock_cluster_server(
            "404 Not Found",
            not_found_status_json(),
            "201 Created",
            sample_cluster_json(),
        )
        .await;
        let cfg = LocalConfig {
            default_storage: 12,
            ..LocalConfig::default()
        };

        provision_pgexporter(client, &cfg).await;

        let calls = server.await.expect("server join");
        assert!(calls[0].0.starts_with("GET "));
        assert!(calls[1].0.starts_with("POST "));
        assert_eq!(calls[1].1["spec"]["storage"], 12);
        assert!(calls[1].1["spec"]["pgexporter"].is_object());
        assert!(calls[1].1["spec"]["pgmoneta"].is_null());
    }

    #[tokio::test]
    async fn provision_pgmoneta_patches_existing_cluster_on_200() {
        let (client, server) = mock_cluster_server(
            "200 OK",
            sample_cluster_json(),
            "200 OK",
            sample_cluster_json(),
        )
        .await;
        let cfg = LocalConfig {
            default_pgmoneta_storage: 30,
            ..LocalConfig::default()
        };

        provision_pgmoneta(client, &cfg).await;

        let calls = server.await.expect("server join");
        assert!(calls[0].0.starts_with("GET "));
        assert!(calls[1].0.starts_with("PATCH "));
        assert_eq!(calls[1].1["spec"]["pgmoneta"]["storage"], 30);
    }
}
