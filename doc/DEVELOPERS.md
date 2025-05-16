# Developer guide

## Install pgopr

### Pre-install

#### Basic dependencies

``` sh
dnf install git rust rustfmt rust-srpm rust-std-static cargo make
```

### Install

``` sh
make build
cd target/debug
./pgopr --help
```

## Setup pgopr

Let's give it a try. The basic idea here is that we will use [**pgopr**](https://github.com/pgopr/pgopr), which will control PostgreSQL.

#### Add pgopr user

``` sh
sudo su -
useradd -ms /bin/bash pgopr
passwd pgmoneta
exit
```

#### Add Kubernetes environment

You will need a Kubernetes environment such as

* [kind](https://github.com/kubernetes-sigs/kind)
* [minikube](https://github.com/kubernetes/minikube/)

along with their dependencies.

### 2. pgopr

Open a new window, switch to the `pgopr` user. This section will always operate within this user space.

``` sh
sudo su -
su - pgopr
```

#### Using pgopr

Open a new terminal and log in with `pgopr

``` sh
pgopr --help
```

and you can use the commands.

## End

Now that we've attempted to use `pgopr`, take a moment to relax. There are a few things we need to pay attention to:

1. Always format your code when you make modifications using the provided rustfmt.sh script.

## Code Formatting

The project includes a simple rustfmt.sh script to ensure consistent code formatting using Rust's built-in formatter (rustfmt).

### Setting up the rustfmt.sh script

1. Make sure the script is executable:

```sh
chmod +x rustfmt.sh
```

2. Running the formatter
```sh
./rustfmt.sh
   ```

This script will format all Rust files in the project according to the project's formatting guidelines. Always run this script before committing changes to ensure consistent code style across the codebase.

## Rust programming

[**pgopr**](https://github.com/pgopr/pgopr) is developed using the [Rust programming language](https://en.wikipedia.org/wiki/Rust_(programming_language) so it is a good idea to have some knowledge about the language before you begin to make changes.

There are books like,

* [Rust](https://doc.rust-lang.org/book/)

that can help you

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

### Add upstream

Do

```sh
cd pgopr
git remote add upstream https://github.com/pgopr/pgopr.git
```

### Do a work branch

```sh
git checkout -b mywork main
```

### Make the changes

Remember to verify the compile and execution of the code

### AUTHORS

Remember to add your name to the following files,

```
AUTHORS
Cargo.toml
```

in your first pull request

### Multiple commits

If you have multiple commits on your branch then squash them

``` sh
git rebase -i HEAD~2
```

for example. It is `p` for the first one, then `s` for the rest

### Rebase

Always rebase

``` sh
git fetch upstream
git rebase -i upstream/main
```

### Force push

When you are done with your changes force push your branch

``` sh
git push -f origin mywork
```

and then create a pull requests for it

### Repeat

Based on feedback keep making changes, squashing, rebasing and force pushing

### Undo

Normally you can reset to an earlier commit using `git reset <commit hash> --hard`.
But if you accidentally squashed two or more commits, and you want to undo that,
you need to know where to reset to, and the commit seems to have lost after you rebased.

But they are not actually lost - using `git reflog`, you can find every commit the HEAD pointer
has ever pointed to. Find the commit you want to reset to, and do `git reset --hard`.
