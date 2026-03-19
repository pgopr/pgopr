\newpage

# Quick start

Make sure that [**pgopr**][pgopr] is installed and in your path by using `pgopr --help`. You should see

``` console
pgopr 0.2.0
  PostgreSQL operator for Kubernetes

Usage:
  pgopr [SUBCOMMAND]

Subcommands:
  install       Install the operator
  provision     Provision a component
  retire        Retire a component
  uninstall     Uninstall the operator
  completion    Generate a shell completion file
  generate      Generate YAML resources
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help    Print help
  -V, --version Print version
```

If you encounter any issues following the above steps, you can refer to the **Installation** chapter to see how to install or compile pgopr on your system.

## Prerequisites

You need a running Kubernetes environment, such as [kind](https://kind.sigs.k8s.io/) or [minikube](https://github.com/kubernetes/minikube/).

Ensure you have `kubectl` installed and configured to communicate with your cluster.

## Deployment

### Install the Operator

First, install the `pgopr` operator into your Kubernetes cluster:

``` sh
pgopr install
```

This will deploy the necessary Custom Resource Definitions (CRDs) and the operator control plane.

### Provision a Primary Instance

Provision a PostgreSQL 17 primary instance using the `provision` subcommand:

``` sh
pgopr provision primary
```

This will create the PostgreSQL cluster, services, and persistent storage.

## Accessing the Database

To connect to your newly provisioned database, you'll need to identify its instance and forward the network port.

### Check Services

List the services created by the operator:

``` sh
kubectl get services
```

### Port-forwarding

Forward your local port `5432` to the PostgreSQL pod:

``` sh
kubectl port-forward svc/postgresql 5432:5432
```

### Connect with psql

Use `psql` to connect to the database (the default password is `mypass`):

``` sh
psql -h localhost -p 5432 -U myuser mydb
```

## Administration

[**pgopr**][pgopr] itself acts as the administration tool. All lifecycle operations are managed via the CLI.

### Generate YAML resources

If you prefer to manage the Kubernetes resources directly with `kubectl`, you can generate the YAML definitions instead of using `provision`:

``` sh
pgopr generate primary
```

### Shell Completion

To enable shell completion for your environment (e.g., bash):

``` sh
pgopr completion bash > ~/pgopr_completion.sh
echo "source ~/pgopr_completion.sh" >> ~/.bashrc
```

## Troubleshooting

### Persistence Issues

If the operator fails to provision storage, check if your cluster supports dynamic volume provisioning. On local clusters like `kind`, ensure `inotify` limits are configured correctly:

```bash
sudo sysctl fs.inotify.max_user_watches=524288
sudo sysctl fs.inotify.max_user_instances=512
```

### Cluster Connectivity

Verify the operator's pod logs if the primary instance does not reach a "Ready" state:

``` sh
kubectl logs -l app=pgopr
```

## Next Steps

Explore the full capabilities of `pgopr`:
* Read the manual
* Configure custom PostgreSQL settings
* Manage multiple clusters
* Integrate with Prometheus for metrics

## Closing

The [pgopr](https://github.com/pgopr/pgopr) community hopes that you find the project interesting.

All contributions are most welcome!

Please, consult our [Code of Conduct][conduct] policies for interacting in our community.

Consider giving the project a [star][star] on [GitHub][pgopr] if you find it useful.
