# Developer Guide

This document describes the development workflow for **pgopr**.

It is intended for developers who want to build, test, debug, or extend the project.

- For contribution rules and PR workflow, see [CONTRIBUTING.md](../CONTRIBUTING.md)
- For user setup and runtime configuration, see [GETTING_STARTED.md](GETTING_STARTED.md)

---

## Prerequisites

- Rust 1.85+
- Rust toolchain (stable), preferably installed via [rustup](https://rustup.rs)
- `cargo` (included with Rust)
- `git`
- A running Kubernetes environment (e.g., [kind](https://kind.sigs.k8s.io/))

On Linux, some distributions provide useful system packages:

```bash
# Fedora / RHEL
sudo dnf install git rustfmt clippy
```

Using **rustup** is recommended for consistent toolchain management across platforms.

---

## Building

All build tasks are handled by Cargo (or via the provided `Makefile`).

### Debug build

```bash
make build
# or
cargo build
```

### Release build

```bash
make release
# or
cargo build --release
```

Binaries are placed in:

* `target/debug/`
* `target/release/`

---

## Generate User and Developer Guide

This process is optional. If you choose not to generate the PDF and HTML manuals, you can skip these steps.

### Download dependencies

```bash
# Fedora
sudo dnf install pandoc texlive-scheme-basic
```

### Setup Eisvogel

Locate your user data directory (`$HOME/.local/share/pandoc` on Linux) and install the Eisvogel template:

```bash
wget https://github.com/Wandmalfarbe/pandoc-latex-template/releases/download/v3.4.0/Eisvogel-3.4.0.tar.gz
tar -xzf Eisvogel-3.4.0.tar.gz
mkdir -p $HOME/.local/share/pandoc/templates
mv Eisvogel-3.4.0/eisvogel.latex $HOME/.local/share/pandoc/templates/
```

### Build documentation

Run from the project root:

```bash
make doc
```

---

## Environment Setup

### Add pgopr user

For isolated testing on Linux:

```bash
sudo su -
useradd -ms /bin/bash pgopr
passwd pgopr
exit
```

### Kubernetes Environment

You will need a Kubernetes environment along with their dependencies:

* [kind](https://github.com/kubernetes-sigs/kind)
* [minikube](https://github.com/kubernetes/minikube/)

---

## Formatting and Linting

Code formatting and linting are enforced by CI.

### Format code

To automatically format your Rust source code:

```bash
cargo fmt --all
```

### Run Clippy

To run the Rust linter and check for issues:

```bash
cargo clippy
```

---

## Testing

### Run all tests

```bash
cargo test
```

### Run tests with output

```bash
cargo test -- --nocapture
```

---

## Basic git guide

Here are some links that will help you

* [How to Squash Commits in Git](https://www.git-tower.com/learn/git/faq/git-squash)
* [ProGit book](https://github.com/progit/progit2/releases)

### Start by forking the repository

This is done by the "Fork" button on GitHub.

### Clone your repository locally

This is done by

```sh
git clone git@github.com:<username>/pgopr.git
```

### Add upstream remote

```sh
cd pgopr
git remote add upstream https://github.com/pgopr/pgopr.git
```

### Create a work branch

```sh
git checkout -b mywork main
```

### Commit message format

Include the issue number in your commit messages: `[#issue_number] commit message`.

### SQUASH AND REBASE

Before submitting a Pull Request, ensure your history is clean:

```bash
# Squash commits
git rebase -i HEAD~N
# Rebase on main
git fetch upstream
git rebase -i upstream/main
```

---

## Contributing Notes

* Add yourself to the `AUTHORS` and `doc/manual/en/97-acknowledgement.md` files in your first pull request
* Follow the workflow described in [CONTRIBUTING.md](../CONTRIBUTING.md)

---

Thank you for contributing to **pgopr**!
