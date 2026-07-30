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

pub async fn handle_deploy(image: &str, target_ns: &str, dry_run: bool, wait: bool) {
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
        return;
    }

    let client = k8s::k8s_client().await;

    // 1. CRD (reuse existing)
    let _ = crate::crd::crd_deploy(client.clone()).await;

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
        .expect("Failed to create namespace");

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
        .expect("Failed to create ServiceAccount");

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
        .expect("Failed to create Role");

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
        .expect("Failed to create RoleBinding");

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
        .expect("Failed to create Deployment");

    info!(
        "deployed pgopr v{}, watching namespace {}",
        env!("CARGO_PKG_VERSION"),
        target_ns
    );

    if wait {
        let wait_api: Api<Deployment> = Api::namespaced(client.clone(), OPERATOR_NAMESPACE);
        let condition = conditions::is_deployment_completed();
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            await_condition(wait_api, OPERATOR_NAME, condition),
        )
        .await
        .expect("Timed out waiting for deployment to become ready");
        info!("operator deployment is ready");
    }
}

pub async fn handle_undeploy(target_ns: &str, dry_run: bool, delete_ns: bool) {
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
        return;
    }

    // 1. Delete Deployment
    let deploy_api: Api<Deployment> = Api::namespaced(client.clone(), OPERATOR_NAMESPACE);
    match deploy_api
        .delete(OPERATOR_NAME, &DeleteParams::default())
        .await
    {
        Ok(_) => info!("deleted Deployment"),
        Err(kube::Error::Api(e)) if e.code == 404 => {} // already gone
        Err(e) => panic!("Failed to delete Deployment: {:?}", e),
    }

    // 2. Delete RoleBinding
    let rb_api: Api<RoleBinding> = Api::namespaced(client.clone(), target_ns);
    match rb_api.delete(OPERATOR_NAME, &DeleteParams::default()).await {
        Ok(_) => info!("deleted RoleBinding"),
        Err(kube::Error::Api(e)) if e.code == 404 => {}
        Err(e) => panic!("Failed to delete RoleBinding: {:?}", e),
    }

    // 3. Delete Role
    let role_api: Api<Role> = Api::namespaced(client.clone(), target_ns);
    match role_api
        .delete(OPERATOR_NAME, &DeleteParams::default())
        .await
    {
        Ok(_) => info!("deleted Role"),
        Err(kube::Error::Api(e)) if e.code == 404 => {}
        Err(e) => panic!("Failed to delete Role: {:?}", e),
    }

    // 4. Delete ServiceAccount
    let sa_api: Api<ServiceAccount> = Api::namespaced(client.clone(), OPERATOR_NAMESPACE);
    match sa_api.delete(OPERATOR_NAME, &DeleteParams::default()).await {
        Ok(_) => info!("deleted ServiceAccount"),
        Err(kube::Error::Api(e)) if e.code == 404 => {}
        Err(e) => panic!("Failed to delete ServiceAccount: {:?}", e),
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
            Err(e) => panic!("Failed to delete Namespace: {:?}", e),
        }
    }

    info!("undeploy complete");
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
}
