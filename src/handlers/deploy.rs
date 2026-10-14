/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */
use crate::k8s;
use k8s_openapi::api::{
    apps::v1::{Deployment, DeploymentSpec},
    core::v1::{Container, EnvVar, Namespace, PodSpec, PodTemplateSpec, ServiceAccount},
    rbac::v1::{PolicyRule, Role, RoleBinding, RoleRef, Subject},
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::api::ObjectMeta;
use kube::{
    Api,
    api::{DeleteParams, Patch, PatchParams},
    runtime::wait::{await_condition, conditions},
};
use log::info;
use std::collections::BTreeMap;

const OPERATOR_NAME: &str = "pgopr-operator";
const OPERATOR_NAMESPACE: &str = "pgopr-system";
/// Seconds to wait for the operator Deployment to become ready (`--wait`).
const OPERATOR_READY_TIMEOUT_SECS: u64 = 120;

/// CLI-specific error for deployment-related operations (`deploy` / `undeploy`).
#[derive(Debug, thiserror::Error)]
pub enum DeployError {
    #[error("failed to {operation} {resource}: {source}")]
    Api {
        operation: String,
        resource: String,
        #[source]
        source: kube::Error,
    },

    /// Deploying (create or patch) the pgopr CRD failed.
    #[error("failed to deploy pgopr CRD: {source}")]
    CrdDeploy {
        #[source]
        source: kube::Error,
    },

    #[error("failed waiting for operator Deployment {namespace}/{name} to become ready: {source}")]
    WaitReady {
        namespace: String,
        name: String,
        #[source]
        source: kube::runtime::wait::Error,
    },

    #[error(
        "timed out after {timeout_secs}s waiting for operator Deployment {namespace}/{name} to become ready"
    )]
    WaitTimeout {
        namespace: String,
        name: String,
        timeout_secs: u64,
        #[source]
        source: tokio::time::error::Elapsed,
    },
}

/// Builds the Namespace object for the operator control plane.
///
/// The namespace (default: `pgopr-system`) hosts all operator-internal
/// resources: the ServiceAccount, the operator Deployment, and related
/// RBAC artifacts. It is separate from the namespace the operator watches.
fn build_operator_namespace() -> Namespace {
    let mut labels = BTreeMap::new();
    labels.insert("app".to_string(), OPERATOR_NAME.to_string());
    labels.insert(
        "pgopr.io/operator-version".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    );
    Namespace {
        metadata: ObjectMeta {
            name: Some(OPERATOR_NAMESPACE.to_string()),
            labels: Some(labels),
            ..ObjectMeta::default()
        },
        ..Namespace::default()
    }
}
/// Builds the ServiceAccount object for the operator Pod.
///
/// The operator Deployment runs under this identity. RBAC permissions
/// (Role + RoleBinding) are granted to this ServiceAccount, not to
/// individual Pods.
fn build_operator_service_account() -> ServiceAccount {
    let mut labels = BTreeMap::new();
    labels.insert("app".to_string(), OPERATOR_NAME.to_string());
    labels.insert(
        "pgopr.io/operator-version".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    );

    ServiceAccount {
        metadata: ObjectMeta {
            name: Some(OPERATOR_NAME.to_string()),
            namespace: Some(OPERATOR_NAMESPACE.to_string()),
            labels: Some(labels),
            ..ObjectMeta::default()
        },
        ..ServiceAccount::default()
    }
}
/// Builds a namespaced Role granting the operator permission to manage
/// resources in the target namespace.
///
/// The role covers three groups of resources:
/// - pgopr custom resources (CRD level: get, list, watch, patch, update)
/// - core/v1 resources (pods, services, PVCs, secrets, configmaps: full CRUD)
/// - apps/v1 resources (deployments: full CRUD)
///
/// # Arguments
/// - `target_ns` - Namespace where the operator will watch and manage resources
fn build_operator_role(target_ns: &str) -> Role {
    let mut labels = BTreeMap::new();
    labels.insert("app".to_string(), OPERATOR_NAME.to_string());
    labels.insert(
        "pgopr.io/operator-version".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    );

    Role {
        metadata: ObjectMeta {
            name: Some(OPERATOR_NAME.to_string()),
            namespace: Some(target_ns.to_string()),
            labels: Some(labels),
            ..ObjectMeta::default()
        },
        rules: Some(vec![
            PolicyRule {
                api_groups: Some(vec!["pgopr.io".to_string()]),
                resources: Some(vec![
                    "pgoprs".to_string(),
                    "pgoprs/status".to_string(),
                    "pgoprs/finalizers".to_string(),
                ]),
                verbs: vec![
                    "get".to_string(),
                    "list".to_string(),
                    "watch".to_string(),
                    "patch".to_string(),
                    "update".to_string(),
                ],
                ..PolicyRule::default()
            },
            PolicyRule {
                api_groups: Some(vec!["".to_string()]),
                resources: Some(vec![
                    "pods".to_string(),
                    "services".to_string(),
                    "persistentvolumeclaims".to_string(),
                    "secrets".to_string(),
                    "configmaps".to_string(),
                ]),
                verbs: vec![
                    "get".to_string(),
                    "list".to_string(),
                    "watch".to_string(),
                    "create".to_string(),
                    "update".to_string(),
                    "patch".to_string(),
                    "delete".to_string(),
                ],
                ..PolicyRule::default()
            },
            PolicyRule {
                api_groups: Some(vec!["apps".to_string()]),
                resources: Some(vec!["deployments".to_string()]),
                verbs: vec![
                    "get".to_string(),
                    "list".to_string(),
                    "watch".to_string(),
                    "create".to_string(),
                    "update".to_string(),
                    "patch".to_string(),
                    "delete".to_string(),
                ],
                ..PolicyRule::default()
            },
        ]),
    }
}
/// Builds a RoleBinding that connects the operator's ServiceAccount
/// (in `pgopr-system`) to the operator's Role (in the target namespace).
///
/// This cross-namespace binding is the standard Kubernetes operator pattern:
/// the identity lives in `pgopr-system`, but permissions are granted in the
/// namespace being managed.
///
/// # Arguments
/// - `target_ns` - Namespace where the Role was created
fn build_operator_role_binding(target_ns: &str) -> RoleBinding {
    let mut labels = BTreeMap::new();
    labels.insert("app".to_string(), OPERATOR_NAME.to_string());
    labels.insert(
        "pgopr.io/operator-version".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    );

    RoleBinding {
        metadata: ObjectMeta {
            name: Some(OPERATOR_NAME.to_string()),
            namespace: Some(target_ns.to_string()),
            labels: Some(labels),
            ..ObjectMeta::default()
        },
        role_ref: RoleRef {
            api_group: "rbac.authorization.k8s.io".to_string(),
            kind: "Role".to_string(),
            name: OPERATOR_NAME.to_string(),
        },
        subjects: Some(vec![Subject {
            kind: "ServiceAccount".to_string(),
            name: OPERATOR_NAME.to_string(),
            namespace: Some(OPERATOR_NAMESPACE.to_string()),
            ..Subject::default()
        }]),
    }
}
/// Builds the Deployment object for the operator controller Pod.
///
/// The Deployment runs one replica of the operator binary with no open ports
/// (it connects outbound to the Kubernetes API). The `PGOPR_TARGET_NAMESPACE`
/// env var tells the controller which namespace to watch.
///
/// # Arguments
/// - `image` - Container image reference (default: ghcr.io/pgopr/operator:latest)
/// - `target_ns` - Namespace the operator should watch
fn build_operator_deployment(image: &str, target_ns: &str) -> Deployment {
    let mut labels = BTreeMap::new();
    labels.insert("app".to_string(), OPERATOR_NAME.to_string());
    labels.insert(
        "pgopr.io/operator-version".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    );

    let selector_labels = {
        let mut m = BTreeMap::new();
        m.insert("app".to_string(), OPERATOR_NAME.to_string());
        m
    };

    Deployment {
        metadata: ObjectMeta {
            name: Some(OPERATOR_NAME.to_string()),
            namespace: Some(OPERATOR_NAMESPACE.to_string()),
            labels: Some(labels.clone()),
            ..ObjectMeta::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(1),
            selector: LabelSelector {
                match_labels: Some(selector_labels),
                ..LabelSelector::default()
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels),
                    ..ObjectMeta::default()
                }),
                spec: Some(PodSpec {
                    service_account_name: Some(OPERATOR_NAME.to_string()),
                    containers: vec![Container {
                        name: OPERATOR_NAME.to_string(),
                        image: Some(image.to_string()),
                        image_pull_policy: Some("IfNotPresent".to_string()),
                        env: Some(vec![EnvVar {
                            name: "PGOPR_TARGET_NAMESPACE".to_string(),
                            value: Some(target_ns.to_string()),
                            ..EnvVar::default()
                        }]),
                        ports: Some(vec![]), // operator has no open ports
                        resources: Some(k8s_openapi::api::core::v1::ResourceRequirements {
                            requests: Some({
                                let mut m = BTreeMap::new();
                                m.insert(
                                    "cpu".to_string(),
                                    k8s_openapi::apimachinery::pkg::api::resource::Quantity(
                                        "100m".to_string(),
                                    ),
                                );
                                m.insert(
                                    "memory".to_string(),
                                    k8s_openapi::apimachinery::pkg::api::resource::Quantity(
                                        "128Mi".to_string(),
                                    ),
                                );
                                m
                            }),
                            ..k8s_openapi::api::core::v1::ResourceRequirements::default()
                        }),
                        ..Container::default()
                    }],
                    ..PodSpec::default()
                }),
            },
            ..DeploymentSpec::default()
        }),
        ..Deployment::default()
    }
}

/// Deploys the operator control plane into the cluster.
///
/// Creates or updates: CRD, Namespace, ServiceAccount, Role, RoleBinding,
/// and Deployment. All resources are applied via Server-Side Apply.
///
/// # Arguments
/// - `image` - Container image for the operator
/// - `target_ns` - Namespace the operator will watch
/// - `dry_run` - If true, print generated YAML and exit without API calls
/// - `wait` - If true, block until the operator Deployment is ready
pub async fn handle_deploy(
    image: &str,
    target_ns: &str,
    dry_run: bool,
    wait: bool,
) -> Result<(), DeployError> {
    super::print_header();

    if dry_run {
        // Serialize each resource to YAML and print
        let ns = build_operator_namespace();
        println!("---\n{}", serde_yaml::to_string(&ns).unwrap());
        let sa = build_operator_service_account();
        println!("---\n{}", serde_yaml::to_string(&sa).unwrap());
        let role = build_operator_role(target_ns);
        println!("---\n{}", serde_yaml::to_string(&role).unwrap());
        let rb = build_operator_role_binding(target_ns);
        println!("---\n{}", serde_yaml::to_string(&rb).unwrap());
        let deploy = build_operator_deployment(image, target_ns);
        println!("---\n{}", serde_yaml::to_string(&deploy).unwrap());
        return Ok(());
    }

    let client = k8s::k8s_client().await;

    // 1. CRD (reuse existing)
    crate::crd::crd_deploy(client.clone())
        .await
        .map_err(|source| DeployError::CrdDeploy { source })?;

    // 2. Namespace
    let ns_api: Api<Namespace> = Api::all(client.clone());
    let ns = build_operator_namespace();
    ns_api
        .patch(
            OPERATOR_NAMESPACE,
            &PatchParams::apply("pgopr-deploy-manager"),
            &Patch::Apply(&ns),
        )
        .await
        .map_err(|source| DeployError::Api {
            operation: "apply".to_string(),
            resource: format!("Namespace {}", OPERATOR_NAMESPACE),
            source,
        })?;

    // 3. ServiceAccount
    let sa_api: Api<ServiceAccount> = Api::namespaced(client.clone(), OPERATOR_NAMESPACE);
    let sa = build_operator_service_account();
    sa_api
        .patch(
            OPERATOR_NAME,
            &PatchParams::apply("pgopr-deploy-manager"),
            &Patch::Apply(&sa),
        )
        .await
        .map_err(|source| DeployError::Api {
            operation: "apply".to_string(),
            resource: format!("ServiceAccount {}/{}", OPERATOR_NAMESPACE, OPERATOR_NAME),
            source,
        })?;

    // 4. Role
    let role_api: Api<Role> = Api::namespaced(client.clone(), target_ns);
    let role = build_operator_role(target_ns);
    role_api
        .patch(
            OPERATOR_NAME,
            &PatchParams::apply("pgopr-deploy-manager"),
            &Patch::Apply(&role),
        )
        .await
        .map_err(|source| DeployError::Api {
            operation: "apply".to_string(),
            resource: format!("Role {}/{}", target_ns, OPERATOR_NAME),
            source,
        })?;

    // 5. RoleBinding
    let rb_api: Api<RoleBinding> = Api::namespaced(client.clone(), target_ns);
    let rb = build_operator_role_binding(target_ns);
    rb_api
        .patch(
            OPERATOR_NAME,
            &PatchParams::apply("pgopr-deploy-manager"),
            &Patch::Apply(&rb),
        )
        .await
        .map_err(|source| DeployError::Api {
            operation: "apply".to_string(),
            resource: format!("RoleBinding {}/{}", target_ns, OPERATOR_NAME),
            source,
        })?;

    // 6. Deployment
    let deploy_api: Api<Deployment> = Api::namespaced(client.clone(), OPERATOR_NAMESPACE);
    let deploy = build_operator_deployment(image, target_ns);
    deploy_api
        .patch(
            OPERATOR_NAME,
            &PatchParams::apply("pgopr-deploy-manager"),
            &Patch::Apply(&deploy),
        )
        .await
        .map_err(|source| DeployError::Api {
            operation: "apply".to_string(),
            resource: format!("Deployment {}/{}", OPERATOR_NAMESPACE, OPERATOR_NAME),
            source,
        })?;

    info!(
        "deployed pgopr v{}, watching namespace {}",
        env!("CARGO_PKG_VERSION"),
        target_ns
    );

    if wait {
        let wait_api: Api<Deployment> = Api::namespaced(client.clone(), OPERATOR_NAMESPACE);
        let condition = conditions::is_deployment_completed();
        let ready = tokio::time::timeout(
            std::time::Duration::from_secs(OPERATOR_READY_TIMEOUT_SECS),
            await_condition(wait_api, OPERATOR_NAME, condition),
        )
        .await
        .map_err(|source| DeployError::WaitTimeout {
            namespace: OPERATOR_NAMESPACE.to_string(),
            name: OPERATOR_NAME.to_string(),
            timeout_secs: OPERATOR_READY_TIMEOUT_SECS,
            source,
        })?;
        ready.map_err(|source| DeployError::WaitReady {
            namespace: OPERATOR_NAMESPACE.to_string(),
            name: OPERATOR_NAME.to_string(),
            source,
        })?;
        info!("operator deployment is ready");
    }

    Ok(())
}

/// Removes the operator control plane from the cluster.
///
/// Deletes: Deployment, RoleBinding, Role, ServiceAccount. Optionally
/// deletes the `pgopr-system` Namespace. The CRD and all PgOpr resources
/// (and their running databases) are left intact.
///
/// A `404 Not Found` during deletion is treated as success (already gone).
///
/// # Arguments
/// - `target_ns` - Namespace the operator was watching
/// - `dry_run` - If true, print what would be deleted and exit
/// - `delete_ns` - If true, also delete the pgopr-system namespace
pub async fn handle_undeploy(
    target_ns: &str,
    dry_run: bool,
    delete_ns: bool,
) -> Result<(), DeployError> {
    super::print_header();
    let client = k8s::k8s_client().await;

    if dry_run {
        println!(
            "Would delete Deployment {}/{}",
            OPERATOR_NAMESPACE, OPERATOR_NAME
        );
        println!("Would delete RoleBinding {}/{}", target_ns, OPERATOR_NAME);
        println!("Would delete Role {}/{}", target_ns, OPERATOR_NAME);
        println!(
            "Would delete ServiceAccount {}/{}",
            OPERATOR_NAMESPACE, OPERATOR_NAME
        );
        if delete_ns {
            println!("Would delete Namespace {}", OPERATOR_NAMESPACE);
        }
        return Ok(());
    }

    // 1. Delete Deployment
    let deploy_api: Api<Deployment> = Api::namespaced(client.clone(), OPERATOR_NAMESPACE);
    match deploy_api
        .delete(OPERATOR_NAME, &DeleteParams::default())
        .await
    {
        Ok(_) => info!("deleted Deployment"),
        Err(kube::Error::Api(e)) if e.code == 404 => {} // already gone
        Err(source) => {
            return Err(DeployError::Api {
                operation: "delete".to_string(),
                resource: format!("Deployment {}/{}", OPERATOR_NAMESPACE, OPERATOR_NAME),
                source,
            });
        }
    }

    // 2. Delete RoleBinding
    let rb_api: Api<RoleBinding> = Api::namespaced(client.clone(), target_ns);
    match rb_api.delete(OPERATOR_NAME, &DeleteParams::default()).await {
        Ok(_) => info!("deleted RoleBinding"),
        Err(kube::Error::Api(e)) if e.code == 404 => {}
        Err(source) => {
            return Err(DeployError::Api {
                operation: "delete".to_string(),
                resource: format!("RoleBinding {}/{}", target_ns, OPERATOR_NAME),
                source,
            });
        }
    }

    // 3. Delete Role
    let role_api: Api<Role> = Api::namespaced(client.clone(), target_ns);
    match role_api
        .delete(OPERATOR_NAME, &DeleteParams::default())
        .await
    {
        Ok(_) => info!("deleted Role"),
        Err(kube::Error::Api(e)) if e.code == 404 => {}
        Err(source) => {
            return Err(DeployError::Api {
                operation: "delete".to_string(),
                resource: format!("Role {}/{}", target_ns, OPERATOR_NAME),
                source,
            });
        }
    }

    // 4. Delete ServiceAccount
    let sa_api: Api<ServiceAccount> = Api::namespaced(client.clone(), OPERATOR_NAMESPACE);
    match sa_api.delete(OPERATOR_NAME, &DeleteParams::default()).await {
        Ok(_) => info!("deleted ServiceAccount"),
        Err(kube::Error::Api(e)) if e.code == 404 => {}
        Err(source) => {
            return Err(DeployError::Api {
                operation: "delete".to_string(),
                resource: format!("ServiceAccount {}/{}", OPERATOR_NAMESPACE, OPERATOR_NAME),
                source,
            });
        }
    }

    // 5. Delete Namespace (optional)
    if delete_ns {
        let ns_api: Api<Namespace> = Api::all(client);
        match ns_api
            .delete(OPERATOR_NAMESPACE, &DeleteParams::default())
            .await
        {
            Ok(_) => info!("deleted Namespace"),
            Err(kube::Error::Api(e)) if e.code == 404 => {}
            Err(source) => {
                return Err(DeployError::Api {
                    operation: "delete".to_string(),
                    resource: format!("Namespace {}", OPERATOR_NAMESPACE),
                    source,
                });
            }
        }
    }

    info!("undeploy complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_operator_namespace_labels() {
        let ns = build_operator_namespace();
        let meta = ns.metadata;
        assert_eq!(meta.name.as_deref(), Some("pgopr-system"));
        let labels = meta.labels.unwrap();
        assert_eq!(labels.get("app").unwrap(), "pgopr-operator");
    }

    #[test]
    fn test_build_operator_role_rules() {
        let role = build_operator_role("default");
        let rules = role.rules.unwrap();
        assert_eq!(rules.len(), 3);

        // Rule 1: CRD access
        assert_eq!(rules[0].api_groups.as_ref().unwrap()[0], "pgopr.io");
        assert!(rules[0].verbs.contains(&"watch".to_string()));

        // Rule 2: Core resources
        assert!(
            rules[1]
                .resources
                .as_ref()
                .unwrap()
                .contains(&"pods".to_string())
        );
        assert!(rules[1].verbs.contains(&"delete".to_string()));

        // Rule 3: Apps
        assert!(rules[2].verbs.contains(&"patch".to_string()));
    }

    #[test]
    fn test_build_operator_role_binding() {
        let rb = build_operator_role_binding("test-ns");
        assert_eq!(rb.metadata.namespace.as_deref(), Some("test-ns"));
        assert_eq!(rb.role_ref.name, "pgopr-operator");
        let subject = &rb.subjects.unwrap()[0];
        assert_eq!(subject.namespace.as_deref(), Some("pgopr-system"));
    }

    #[test]
    fn test_build_operator_deployment() {
        let deploy = build_operator_deployment("my-image:latest", "my-ns");
        let spec = deploy.spec.unwrap();
        assert_eq!(spec.replicas, Some(1));
        let container = &spec.template.spec.unwrap().containers[0];
        assert_eq!(container.image.as_deref(), Some("my-image:latest"));
        let env = container.env.as_ref().unwrap();
        assert!(env.iter().any(|e| {
            e.name == "PGOPR_TARGET_NAMESPACE" && e.value.as_deref() == Some("my-ns")
        }));
    }

    fn test_api_status(code: u16) -> kube::Error {
        let status = kube::core::Status {
            code,
            message: "forbidden".to_string(),
            reason: "Forbidden".to_string(),
            ..Default::default()
        };
        kube::Error::Api(status.boxed())
    }

    #[test]
    fn test_deploy_error_api_display() {
        let err = DeployError::Api {
            operation: "apply".to_string(),
            resource: "Deployment pgopr-system/pgopr-operator".to_string(),
            source: test_api_status(403),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("failed to apply Deployment pgopr-system/pgopr-operator"),
            "unexpected message: {msg}"
        );
        assert!(msg.contains("forbidden"), "underlying error lost: {msg}");
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn test_deploy_error_crd_display() {
        let err = DeployError::CrdDeploy {
            source: test_api_status(403),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("failed to deploy pgopr CRD"),
            "unexpected message: {msg}"
        );
        assert!(msg.contains("forbidden"), "underlying error lost: {msg}");
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn test_deploy_error_wait_ready_display() {
        let err = DeployError::WaitReady {
            namespace: OPERATOR_NAMESPACE.to_string(),
            name: OPERATOR_NAME.to_string(),
            source: kube::runtime::wait::Error::ProbeFailed(
                kube::runtime::watcher::Error::NoResourceVersion,
            ),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("pgopr-system/pgopr-operator"),
            "missing resource context: {msg}"
        );
        assert!(std::error::Error::source(&err).is_some());
    }

    #[tokio::test]
    async fn test_deploy_error_wait_timeout_display() {
        let elapsed = tokio::time::timeout(
            std::time::Duration::from_millis(1),
            std::future::pending::<()>(),
        )
        .await
        .expect_err("timeout should elapse");
        let err = DeployError::WaitTimeout {
            namespace: OPERATOR_NAMESPACE.to_string(),
            name: OPERATOR_NAME.to_string(),
            timeout_secs: OPERATOR_READY_TIMEOUT_SECS,
            source: elapsed,
        };
        let msg = err.to_string();
        assert!(
            msg.contains("timed out after 120s"),
            "missing deadline context: {msg}"
        );
        assert!(
            msg.contains("pgopr-system/pgopr-operator"),
            "missing resource context: {msg}"
        );
        assert!(std::error::Error::source(&err).is_some());
    }
}
