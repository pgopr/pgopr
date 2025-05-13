## Generate YAML resources

This tutorial will show you how to creates Kubernetes YAML resource definitions.

### Preface

This tutorial assumes that you have an installation of [**pgopr**](https://github.com/pgopr/pgopr).

See [install pgopr](./01_install_operator.md) for more detail.

### Generate YAML

- Command to create YAML definition for a PostgreSQL primary instance:

```bash
./pgopr generate --type primary
```

- Other valid commands:

```bash
./pgopr generate --type crd         # Generates the Custom Resource Definition
./pgopr generate --type persistent  # Generates the Persistent Volume/Claim definitions
./pgopr generate --type service     # Generates the Service definition
```

the command creates Kubernetes YAML resource definitions for components used by the PostgreSQL operator. By specifying the --type flag, you can generate manifests for resources such as the CRD, primary deployment, persistent volume, or service—useful for inspection or manual application.
