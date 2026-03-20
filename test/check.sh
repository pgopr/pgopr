#!/bin/bash
## Copyright (C) 2026 The pgmoneta community
##
## This program is free software: you can redistribute it and/or modify
## it under the terms of the GNU General Public License as published by
## the Free Software Foundation, either version 3 of the License, or
## (at your option) any later version.
##
## This program is distributed in the hope that it will be useful,
## but WITHOUT ANY WARRANTY; without even the implied warranty of
## MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
## GNU General Public License for more details.
##
## You should have received a copy of the GNU General Public License
## along with this program. If not, see <https://www.gnu.org/licenses/>.
set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
readonly PGOPR_BIN="$PROJECT_ROOT/target/debug/pgopr"

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

test_operator() {
    echo "Running basic tests for the operator (start/stop)..."
    
    cd "$PROJECT_ROOT"
    
    # 1. Install operator CRDs
    echo "Installing pgopr operator CRDs..."
    "$PGOPR_BIN" install

    echo "Starting pgopr operator control loop in the background..."
    "$PGOPR_BIN" &
    PGOPR_PID=$!

    echo "Waiting for operator to initialize..."
    sleep 5

    # 2. Provision a primary instance
    echo "Provisioning primary PostgreSQL instance..."
    "$PGOPR_BIN" provision primary

    echo "Waiting for postgresql deployment to be ready..."
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
    
    # Once the deployment exists, we can use kubectl wait
    kubectl wait --for=condition=Available deployment/postgresql --timeout=60s
    
    echo "PostgreSQL primary is running!"
    kubectl get pods
    kubectl get svc

    # 3. Retire the primary instance
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

    echo "Stopping the pgopr operator process..."
    kill -TERM "$PGOPR_PID" 2>/dev/null || true
    wait "$PGOPR_PID" 2>/dev/null || true

    echo "Operations completed successfully."
}

cleanup_cluster() {
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
