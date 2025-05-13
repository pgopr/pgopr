## Provision a PostgreSQL primary setup

This tutorial will show you how to provision a PostgreSQL primary setup by creating a persistent volume, claim, deployment, and service in the Kubernetes cluster.

### Preface

This tutorial assumes that you have an installation of [**pgopr**](https://github.com/pgopr/pgopr).

See [install pgopr](./01_install_operator.md) for more detail.

### Provision a primary instance

```bash
./pgopr provision primary
```

will provision a PostgreSQL primary instance in a Kubernetes cluster. This includes deploying persistent volumes, a database pod, and an associated service.
