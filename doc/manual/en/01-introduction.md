\newpage

# Introduction

`pgopr` is a PostgreSQL operator for Kubernetes that controls a PostgreSQL cluster and related technologies.

First of all, make sure that `pgopr` is installed and in your path by using `pgopr -h`. You should see

```
pgopr 0.2.0
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

If you don't have `pgopr` in your path see the [Installation](#installation) chapter on how to
compile and install `pgopr` in your system.
