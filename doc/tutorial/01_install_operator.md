## Install pgopr

This tutorial will show you how to do a simple installation of [**pgopr**](https://github.com/pgopr/pgopr).

At the end of this tutorial you will have an operator that controls a PostgreSQL cluster and related technologies using Kubernetes 1.28+.

### Prerequisites

Before installing pgopr, ensure you have the following prerequisites:

- [Kubernetes](https://kubernetes.io/) 1.32 or later
- [Rust](https://www.rust-lang.org/) and [Cargo](https://doc.rust-lang.org/cargo/)
- [kind](https://kind.sigs.k8s.io/) 0.29 or later
- [minikube](https://minikube.sigs.k8s.io/docs/start/)
- [kubectl](https://kubernetes.io/docs/tasks/tools/) (to run commands against Kubernetes cluster)
- PostgreSQL client tools (for database interaction)

For Fedora 42:

```bash
dnf install -y git rust rust-std-static cargo rustfmt rust-analyzer clippy postgresql
```

### Installation

1. Clone the repository:

```bash
git clone https://github.com/pgopr/pgopr.git
cd pgopr
```

2. Build the project:

```bash
cargo build
```

3. The binary will be available at `target/debug/pgopr`

```bash
cd target/debug
```

4. Create a Kubernetes cluster (using [kind](https://kind.sigs.k8s.io/)):

```bash
kind create cluster
```

Output:

```bash
enabling experimental podman provider
Creating cluster "kind" ...
✓ Ensuring node image (kindest/node:v1.33.1) 🖼
✓ Preparing nodes 📦
✓ Writing configuration 📜
✓ Starting control-plane 🕹️
✓ Installing CNI 🔌
✓ Installing StorageClass 💾
Set kubectl context to "kind-kind"
You can now use your cluster with:

kubectl cluster-info --context kind-kind

Not sure what to do next? 😅  Check out https://kind.sigs.k8s.io/docs/user/quick-start/
```

5. Install pgopr:

```bash
./pgopr install
```

Output:

```bash
2025-05-12T22:40:25.576587386-04:00 INFO pgopr - pgopr 0.2.0
2025-05-12T22:40:25.576743213-04:00 INFO pgopr - PostgreSQL operator for Kubernetes
2025-05-12T22:40:35.603058296-04:00 INFO pgopr::crd - Created CRD
```

## Configuration

pgopr uses configuration files in TOML format. The default configuration is loaded from:

- `$HOME/.config/pgopr/config.toml`
- `$HOME/.pgopr/config.toml`

## Troubleshooting

If you encounter any issues:

1. Check that your Kubernetes cluster is running and accessible
2. Verify that you have the correct permissions in your Kubernetes cluster
3. Ensure all prerequisites are installed and up to date
4. Check the logs using `kubectl logs` for the pgopr operator

## Getting Help

- [Ask a question](https://github.com/pgopr/pgopr/discussions)
- [Raise an issue](https://github.com/pgopr/pgopr/issues)
- [Feature request](https://github.com/pgopr/pgopr/issues)

## License

pgopr is licensed under the [Eclipse Public License - v2.0](https://www.eclipse.org/legal/epl-2.0/)
