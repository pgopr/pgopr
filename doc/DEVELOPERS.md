# Developer guide

## Install PostgreSQL

For RPM based distributions such as Fedora and RHEL you can add the
[PostgreSQL YUM repository](https://yum.postgresql.org/) and do the install via

**Fedora 42**

```sh
rpm -Uvh https://download.postgresql.org/pub/repos/yum/reporpms/F-42-x86_64/pgdg-redhat-repo-latest.noarch.rpm
```

**RHEL 9.x / Rocky Linux 9.x**

**x86_64**

```sh
dnf install https://dl.fedoraproject.org/pub/epel/epel-release-latest-9.noarch.rpm
rpm -Uvh https://download.postgresql.org/pub/repos/yum/reporpms/EL-9-x86_64/pgdg-redhat-repo-latest.noarch.rpm
dnf config-manager --set-enabled crb
```

**aarch64**

```sh
dnf install https://dl.fedoraproject.org/pub/epel/epel-release-latest-9.noarch.rpm
rpm -Uvh https://download.postgresql.org/pub/repos/yum/reporpms/EL-9-aarch64/pgdg-redhat-repo-latest.noarch.rpm
dnf config-manager --set-enabled crb
```

**PostgreSQL 17**

``` sh
dnf -qy module disable postgresql
dnf install -y postgresql17 postgresql17-server postgresql17-contrib postgresql17-libs
```

This will install PostgreSQL 17.

## Install pgopr

### Pre-install

#### Basic dependencies

``` sh
dnf install git rust rustfmt rust-srpm rust-std-static cargo
```

### Check version

You can navigate to `target/debug` and execute `./pgopr --help` to verify the version,:

``` sh
cargo clean
cargo build
cd target/debug
./pgopr --help
```

## Setup pgopr

Let's give it a try. The basic idea here is that we will use two users: one is `postgres`, which will run PostgreSQL, and one is [**pgopr**](https://github.com/pgopr/pgopr), which will run [**pgopr**](https://github.com/pgopr/pgopr) to control PostgreSQL.

In many installations, there is already an operating system user named `postgres` that is used to run the PostgreSQL server. You can use the command

``` sh
getent passwd | grep postgres
```

to check if your OS has a user named postgres. If not use

``` sh
useradd -ms /bin/bash postgres
passwd postgres
```

If the postgres user already exists, don't forget to set its password for convenience.

### 1. postgres

Open a new window, switch to the `postgres` user. This section will always operate within this user space.

``` sh
sudo su -
su - postgres
```

#### Initialize cluster

If you use dnf to install your postgresql, chances are the binary file is in `/usr/bin/`

``` sh
export PATH=/usr/bin:$PATH
initdb -k /tmp/pgsql
```

#### Remove default acess

Remove last lines from `/tmp/pgsql/pg_hba.conf`

``` ini
host    all             all             127.0.0.1/32            trust
host    all             all             ::1/128                 trust
host    replication     all             127.0.0.1/32            trust
host    replication     all             ::1/128                 trust
```

#### Add access for users and a database

Add new lines to `/tmp/pgsql/pg_hba.conf`

``` ini
host    mydb             myuser          127.0.0.1/32            scram-sha-256
host    mydb             myuser          ::1/128                 scram-sha-256
```

#### Set password_encryption

Set `password_encryption` value in `/tmp/pgsql/postgresql.conf` to be `scram-sha-256`

``` sh
password_encryption = scram-sha-256
```

For version 14 and above the default is `scram-sha-256`. Therefore, you should ensure that the value in `/tmp/pgsql/postgresql.conf` matches the value in `/tmp/pgsql/pg_hba.conf`.

#### Set replication level

Set wal_level value in `/tmp/pgsql/postgresql.conf` to be `replica`

``` sh
wal_level = replica
```

#### Start PostgreSQL

``` sh
pg_ctl  -D /tmp/pgsql/ start
```

Here, you may encounter issues such as the port being occupied or permission being denied. If you experience a failure, you can go to `/tmp/pgsql/log` to check the reason.

You can use

``` sh
pg_isready
```

to test

#### Add user and a database

``` sh
export PATH=/usr/pgsql-17/bin:$PATH
createuser -P myuser
createdb -E UTF8 -O myuser mydb
```

#### Verify access

For the user `myuser` (standard) use `mypass`

``` sh
psql -h localhost -p 5432 -U myuser mydb
\q
```

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

1. Since we initialized the database in `/tmp`, the data in this directory might be removed after you go offline, depending on your OS configuration. If you want to make it permanent, choose a different directory.

2. Always format your code when you make modifications using the provided rustfmt.sh script.

## Code Formatting

The project includes a simple rustfmt.sh script to ensure consistent code formatting using Rust's built-in formatter (rustfmt). 

### Setting up the rustfmt.sh script

1. Make sure the script is executable:
   ```sh
   chmod +x rustfmt.sh
   ```

2. Running the formatter:
   ```sh
   ./rustfmt.sh
   ```

This script will format all Rust files in the project according to the project's formatting guidelines. Always run this script before committing changes to ensure consistent code style across the codebase.

## Rust programming

[**pgopr**](https://github.com/pgopr/pgopr) is developed using the [Rust programming language](https://en.wikipedia.org/wiki/Rust_(programming_language) so it is a good
idea to have some knowledge about the language before you begin to make changes.

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
