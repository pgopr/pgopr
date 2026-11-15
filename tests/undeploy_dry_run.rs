/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

use std::process::Command;

#[test]
fn test_undeploy_dry_run_without_kubeconfig() {
    let pgopr = env!("CARGO_BIN_EXE_pgopr");
    let output = Command::new(pgopr)
        .args(["undeploy", "--dry-run"])
        .env_remove("KUBECONFIG")
        .env_remove("KUBERNETES_SERVICE_HOST")
        .env_remove("KUBERNETES_SERVICE_PORT")
        .output()
        .expect("failed to execute pgopr binary");

    assert!(
        output.status.success(),
        "pgopr undeploy --dry-run failed with status {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Would delete Deployment pgopr-system/pgopr-operator"));
    assert!(stdout.contains("Would delete RoleBinding default/pgopr-operator"));
    assert!(stdout.contains("Would delete Role default/pgopr-operator"));
    assert!(stdout.contains("Would delete ServiceAccount pgopr-system/pgopr-operator"));
    assert!(!stdout.contains("Would delete Namespace pgopr-system"));
}

#[test]
fn test_undeploy_dry_run_delete_namespace_without_kubeconfig() {
    let pgopr = env!("CARGO_BIN_EXE_pgopr");
    let output = Command::new(pgopr)
        .args([
            "undeploy",
            "--dry-run",
            "--delete-namespace",
            "--target-namespace",
            "test-ns",
        ])
        .env_remove("KUBECONFIG")
        .env_remove("KUBERNETES_SERVICE_HOST")
        .env_remove("KUBERNETES_SERVICE_PORT")
        .output()
        .expect("failed to execute pgopr binary");

    assert!(
        output.status.success(),
        "pgopr undeploy --dry-run --delete-namespace failed with status {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Would delete Deployment pgopr-system/pgopr-operator"));
    assert!(stdout.contains("Would delete RoleBinding test-ns/pgopr-operator"));
    assert!(stdout.contains("Would delete Role test-ns/pgopr-operator"));
    assert!(stdout.contains("Would delete ServiceAccount pgopr-system/pgopr-operator"));
    assert!(stdout.contains("Would delete Namespace pgopr-system"));
}

#[test]
fn test_undeploy_dry_run_delete_ns_alias_without_kubeconfig() {
    let pgopr = env!("CARGO_BIN_EXE_pgopr");
    let output = Command::new(pgopr)
        .args([
            "undeploy",
            "--dry-run",
            "--delete-ns",
            "--target-namespace",
            "custom-ns",
        ])
        .env_remove("KUBECONFIG")
        .env_remove("KUBERNETES_SERVICE_HOST")
        .env_remove("KUBERNETES_SERVICE_PORT")
        .output()
        .expect("failed to execute pgopr binary");

    assert!(
        output.status.success(),
        "pgopr undeploy --dry-run --delete-ns failed with status {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Would delete Deployment pgopr-system/pgopr-operator"));
    assert!(stdout.contains("Would delete RoleBinding custom-ns/pgopr-operator"));
    assert!(stdout.contains("Would delete Role custom-ns/pgopr-operator"));
    assert!(stdout.contains("Would delete ServiceAccount pgopr-system/pgopr-operator"));
    assert!(stdout.contains("Would delete Namespace pgopr-system"));
}

#[test]
fn test_deploy_dry_run_without_kubeconfig() {
    let pgopr = env!("CARGO_BIN_EXE_pgopr");
    let output = Command::new(pgopr)
        .args(["deploy", "--dry-run"])
        .env_remove("KUBECONFIG")
        .env_remove("KUBERNETES_SERVICE_HOST")
        .env_remove("KUBERNETES_SERVICE_PORT")
        .output()
        .expect("failed to execute pgopr binary");

    assert!(
        output.status.success(),
        "pgopr deploy --dry-run failed with status {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("kind: Deployment"));
    assert!(stdout.contains("kind: Namespace"));
}
