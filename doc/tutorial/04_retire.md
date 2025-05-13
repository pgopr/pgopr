## Retire a PostgreSQL primary setup

This tutorial will show you how to retire the PostgreSQL database.

### Preface

This tutorial assumes that you have an installation of [**pgopr**](https://github.com/pgopr/pgopr) and that a PostgreSQL primary has been provided.

See [install pgopr](./01_install_operator.md) and [provision](./02_provision.md) for more detail.

### Retire a primary instance

```bash
./pgopr retire primary
```

will deprovisions a PostgreSQL primary setup by deleting all associated Kubernetes resources. This includes the removal of the PostgreSQL service, the primary pod, the persistent volume claim (PVC), and the persistent volume (PV), effectively tearing down the deployed database infrastructure from the cluster.
