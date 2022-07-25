# Getting started with pgopr

First of all, make sure that `pgopr` is installed and in your path by
using `pgopr -h`. You should see

```
pgopr 0.1.0
PostgreSQL operator for Kubernetes

USAGE:
    pgopr [SUBCOMMAND]

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

SUBCOMMANDS:
    install       Install the operator
    provision     Provision a component
    retire        Retire a component
    uninstall     Uninstall the operator
    completion    Generate a shell completion file
    generate      Generate YAML resources
    help          Print this message or the help of the given subcommand(s)

pgopr: https://pgopr.github.io/
Report bugs: https://github.com/pgopr/pgopr/issues
```

If you don't have `pgopr` in your path see [README](../README.md) on how to
compile and install `pgopr` in your system.

## Running

We will run `pgopr` using the follow commands

```
KIND_EXPERIMENTAL_PROVIDER=podman kind create cluster
pgopr install
pgopr provision primary
kubectl get services
kubectl port-forward postgresql-XYZ 5432:5432
psql -h localhost -p 5432 -U myuser mydb
```

using `mypass` as the password.

To shutdown the operator use

```
pgopr retire primary
pgopr uninstall
KIND_EXPERIMENTAL_PROVIDER=podman kind delete cluster
```

## Closing

The [pgopr](https://github.com/pgopr/pgopr) community hopes that you find
the project interesting.

Feel free to

* [Ask a question](https://github.com/pgopr/pgopr/discussions)
* [Raise an issue](https://github.com/pgopr/pgopr/issues)
* [Submit a feature request](https://github.com/pgopr/pgopr/issues)
* [Write a code submission](https://github.com/pgopr/pgopr/pulls)

All contributions are most welcome !

Please, consult our [Code of Conduct](../CODE_OF_CONDUCT.md) policies for interacting in our
community.

Consider giving the project a [star](https://github.com/pgopr/pgopr/stargazers) on
[GitHub](https://github.com/pgopr/pgopr/) if you find it useful.
