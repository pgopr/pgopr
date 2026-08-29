\newpage

# Installation

This chapter will show you how to do a simple installation of [**pgopr**](https://github.com/pgopr/pgopr), resulting in an operator that controls a PostgreSQL cluster and related technologies using Kubernetes 1.34+.

### Prerequisites

Before installing pgopr, ensure you have the following prerequisites:

- [Kubernetes](https://kubernetes.io/) 1.34 or later
- [Rust](https://www.rust-lang.org/) and [Cargo](https://doc.rust-lang.org/cargo/)
- [kind](https://kind.sigs.k8s.io/) 0.30 or later
- [minikube](https://minikube.sigs.k8s.io/docs/start/)
- [kubectl](https://kubernetes.io/docs/tasks/tools/) (to run commands against Kubernetes cluster)
- PostgreSQL client tools (for database interaction)

For Fedora 42:

```bash
dnf install -y git rust rust-std-static cargo rustfmt rust-analyzer clippy postgresql
```

### Build

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
[done] Ensuring node image (kindest/node:v1.34.0)
[done] Preparing nodes
[done] Writing configuration
[done] Starting control-plane
[done] Installing CNI
[done] Installing StorageClass
Set kubectl context to "kind-kind"
You can now use your cluster with:

kubectl cluster-info --context kind-kind

Not sure what to do next? Check out https://kind.sigs.k8s.io/docs/user/quick-start/
```

Note, that you may need

```bash
sudo sysctl fs.inotify.max_user_watches=524288
sudo sysctl fs.inotify.max_user_instances=512
```

before you start `kind`. You can edit `/etc/sysctl.conf` to include

```
fs.inotify.max_user_watches = 524288
fs.inotify.max_user_instances = 512
```

and reboot as well.

5. Install the pgopr CRD:

```bash
./pgopr install
```

Output:

```bash
2025-05-12T22:40:25.576587386-04:00 INFO pgopr - pgopr 0.2.0
2025-05-12T22:40:25.576743213-04:00 INFO pgopr - PostgreSQL operator for Kubernetes
2025-05-12T22:40:35.603058296-04:00 INFO pgopr::crd - Created CRD
```

`pgopr install` installs or updates the CustomResourceDefinition. To run the
operator locally for development, start `pgopr` with no subcommand:

```bash
./pgopr
```

To deploy the operator into Kubernetes, use:

```bash
./pgopr deploy --image ghcr.io/pgopr/operator:latest --target-namespace default --wait
```

To inspect the generated operator resources without applying them, use:

```bash
./pgopr deploy --dry-run
```

To remove the in-cluster operator resources:

```bash
./pgopr undeploy --target-namespace default
```

## Configuration

pgopr uses a local TOML configuration file:

- `$HOME/.pgopr/pgopr.toml`

Create it interactively:

```bash
pgopr --init
```

View the current values:

```bash
pgopr config show
```

Update individual values:

```bash
pgopr config set cluster_name postgresql
pgopr config set namespace default
pgopr config set default_storage 5
pgopr config set default_pgmoneta_storage 10
```

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
