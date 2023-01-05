# pgopr: PostgreSQL operator for Kubernetes

`pgopr` is an operator that controls a PostgreSQL cluster and related technologies using [Kubernetes](https://kubernetes.io/) 1.24+.

## Features

* PostgreSQL 13 primary instance
* Command line interface
* CustomResourceDefinition interface
* Shell completion

## Requirements

* [Kubernetes](https://kubernetes.io/) 1.25+

## Technologies

* [Rust](https://www.rust-lang.org/)
* [Cargo](https://doc.rust-lang.org/cargo/)
* [kube-rs](https://github.com/kube-rs/kube-rs)
* [k8s-openapi](https://github.com/Arnavion/k8s-openapi)

## Runtime platforms

* [kind](https://kind.sigs.k8s.io/) 0.17+ ([Guide](https://github.com/pgopr/pgopr-k8s/tree/main/providers/kind))

## Developer

For Fedora 36:

``` bash
dnf install -y git rust rust-std-static cargo rustfmt clippy postgresql
```

``` bash
git clone https://github.com/pgopr/pgopr.git
cd pgopr
cargo build
cd target/debug
kind create cluster
./pgopr install
./pgopr provision primary
kubectl port-forward postgresql-XYZ 5432:5432
psql -h localhost -p 5432 -U myuser --password mydb
./pgopr retire primary
./pgopr uninstall
kind delete cluster
```

Using `mypass` as the password.

## Contributing

Contributions to `pgopr` are managed on [GitHub.com](https://github.com/pgopr/pgopr/)

* [Ask a question](https://github.com/pgopr/pgopr/discussions)
* [Raise an issue](https://github.com/pgopr/pgopr/issues)
* [Feature request](https://github.com/pgopr/pgopr/issues)
* [Code submission](https://github.com/pgopr/pgopr/pulls)

Contributions are most welcome !

Please, consult our [Code of Conduct](./CODE_OF_CONDUCT.md) policies for interacting in our
community.

Consider giving the project a [star](https://github.com/pgopr/pgopr/stargazers) on
[GitHub](https://github.com/pgopr/pgopr/) if you find it useful.

## License

[Eclipse Public License - v2.0](https://www.eclipse.org/legal/epl-2.0/)
