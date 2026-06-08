#!/bin/bash
## Eclipse Public License - v 2.0
##
##   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
##   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
##   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
readonly PGOPR_BIN="$PROJECT_ROOT/target/debug/pgopr"
PGOPR_PID=""

## ================================
## Integration operations
## ================================

setup_cluster() {
    echo "Setting up kind cluster..."
    # Depending on Github actions env, kind might already be installed.
    # We will try to create a cluster.
    if ! command -v kind >/dev/null 2>&1; then
        echo "kind is not installed. Please install kind first."
        exit 1
    fi

    # Check if a cluster already exists
    if ! kind get clusters | grep -q "^kind$"; then
        kind create cluster
    else
        echo "kind cluster already exists."
    fi

    kubectl cluster-info
}

build_operator() {
    echo "Building pgopr operator..."
    cd "$PROJECT_ROOT"
    cargo build
}

stop_operator() {
    if [[ -n "${PGOPR_PID}" ]]; then
        echo "Stopping the pgopr operator process..."
        kill -TERM "$PGOPR_PID" 2>/dev/null || true
        wait "$PGOPR_PID" 2>/dev/null || true
        PGOPR_PID=""
    fi
}

test_operator() {
    echo "Running basic tests for the operator (start/stop)..."
    
    cd "$PROJECT_ROOT"
    
    # 1. Install operator CRDs
    echo "Installing pgopr operator CRDs..."
    "$PGOPR_BIN" install

    echo "Starting pgopr operator control loop in the background..."
    "$PGOPR_BIN" &
    PGOPR_PID=$!
    trap stop_operator RETURN

    echo "Waiting for operator to initialize..."
    sleep 5

    # 2. Provision a primary instance
    echo "Provisioning primary PostgreSQL instance..."
    "$PGOPR_BIN" provision primary

    echo "Waiting for postgresql deployment to be created..."
    # pgopr creates a deployment named "postgresql"
    local count=0
    while ! kubectl get deployment postgresql >/dev/null 2>&1; do
        if [ $count -ge 36 ]; then
            echo "Timeout waiting for postgresql deployment to be created."
            exit 1
        fi
        echo "Waiting for deployment to be created..."
        sleep 5
        count=$((count+1))
    done

    echo "Checking reconciled PostgreSQL resources..."
    kubectl get pv postgresql-pv-volume
    kubectl get pvc postgresql-pv-claim
    kubectl get service postgresql
    
    echo "Verifying PgOpr status..."
    # Check for phase and primary status
    local phase=$(kubectl get pgopr postgresql -o jsonpath='{.status.phase}')
    if [[ "$phase" != "Running" && "$phase" != "Pending" && "$phase" != "Degraded" ]]; then
        echo "Unexpected PgOpr phase: $phase"
        exit 1
    fi

    # Verify that the structured status is present
    if ! kubectl get pgopr postgresql -o jsonpath='{.status.primary.name}' | grep -q "postgresql"; then
        echo "Structured status (primary) missing or incorrect."
        exit 1
    fi

    if ! kubectl get pgopr postgresql -o jsonpath='{.status.storage[0].name}' | grep -q "postgresql"; then
        echo "Structured status (storage) missing or incorrect."
        exit 1
    fi

    echo "PgOpr status verified."

    # 3. Provision pgmoneta
    echo "Provisioning pgmoneta..."
    "$PGOPR_BIN" provision pgmoneta

    echo "Waiting for pgmoneta deployment to be created..."
    local pgmoneta_count=0
    while ! kubectl get deployment postgresql-pgmoneta >/dev/null 2>&1; do
        if [ $pgmoneta_count -ge 36 ]; then
            echo "Timeout waiting for postgresql-pgmoneta deployment."
            exit 1
        fi
        echo "Waiting for pgmoneta deployment..."
        sleep 5
        pgmoneta_count=$((pgmoneta_count+1))
    done

    echo "Checking pgmoneta resources..."
    kubectl get deployment postgresql-pgmoneta
    kubectl get pvc postgresql-pgmoneta-pv-claim
    kubectl get secret postgresql-pgmoneta-secret

    echo "Verifying pgmoneta status..."
    local pgmoneta_ready=$(kubectl get pgopr postgresql -o jsonpath='{.status.pgmoneta.ready}')
    if [[ "$pgmoneta_ready" != "true" && "$pgmoneta_ready" != "false" ]]; then
        echo "Unexpected pgmoneta ready status: $pgmoneta_ready"
        exit 1
    fi

    echo "Checking pgmoneta PV labels..."
    local pv_labels=$(kubectl get pv postgresql-pgmoneta-pv-volume -o jsonpath='{.metadata.labels.pgopr\.io\/component}')
    if [[ "$pv_labels" != "pgmoneta" ]]; then
        echo "Missing pgmoneta component label on PV"
        exit 1
    fi

    # 4. Retire pgmoneta
    echo "Retiring pgmoneta..."
    "$PGOPR_BIN" retire pgmoneta

    echo "Waiting for pgmoneta deployment to be deleted..."
    local pgmoneta_delete_count=0
    while kubectl get deployment postgresql-pgmoneta >/dev/null 2>&1; do
        if [ $pgmoneta_delete_count -ge 24 ]; then
            echo "Timeout waiting for pgmoneta deployment termination."
            exit 1
        fi
        echo "Waiting for pgmoneta deployment termination..."
        sleep 5
        pgmoneta_delete_count=$((pgmoneta_delete_count+1))
    done

    # 5. Retire the primary instance
    echo "Retiring primary PostgreSQL instance..."
    "$PGOPR_BIN" retire primary

    echo "Waiting for postgresql deployment to be deleted..."
    # Wait until it is deleted
    local delete_count=0
    while kubectl get deployment postgresql >/dev/null 2>&1; do
        if [ $delete_count -ge 24 ]; then
            echo "Timeout waiting for deployment termination."
            exit 1
        fi
        echo "Waiting for deployment termination..."
        sleep 5
        delete_count=$((delete_count+1))
    done

    stop_operator

    echo "Operations completed successfully."
}

cleanup_cluster() {
    stop_operator
    echo "Cleaning up kind cluster..."
    kind delete cluster
}

## ================================
## Main script logic
## ================================
usage() {
   echo "Usage: $0 [options] [sub-command]"
   echo "Subcommands:"
   echo " ci             Run full integration suite with CI-specific settings"
   echo " test           Run tests against an existing kind cluster"
   echo " clean          Tear down the kind cluster"
   echo "Examples:"
   echo "  $0 ci          Create cluster, test, clean"
   exit 1
}

SUBCOMMAND=""
while [[ $# -gt 0 ]]; do
case "$1" in
    ci)
        [[ -n "$SUBCOMMAND" ]] && usage
        SUBCOMMAND="ci"
        shift
        ;;
    test)
        [[ -n "$SUBCOMMAND" ]] && usage
        SUBCOMMAND="test"
        shift
        ;;
    clean)
        [[ -n "$SUBCOMMAND" ]] && usage
        SUBCOMMAND="clean"
        shift
        ;;
    -h|--help)
        usage
        ;;
    *)
        echo "Invalid option: $1"
        usage
        ;;
esac
done

if [[ "$SUBCOMMAND" == "ci" ]]; then
    trap cleanup_cluster EXIT
    setup_cluster
    build_operator
    test_operator
    exit 0
fi

if [[ "$SUBCOMMAND" == "test" ]]; then
    build_operator
    test_operator
    exit 0
fi

if [[ "$SUBCOMMAND" == "clean" ]]; then
    cleanup_cluster
    exit 0
fi

if [[ -z "$SUBCOMMAND" ]]; then
    usage
fi
