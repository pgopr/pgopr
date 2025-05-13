## Accessing the PostgreSQL database instance

This tutorial will show you how to access the PostgreSQL database.

### Preface

This tutorial assumes that you have an installation of [**pgopr**](https://github.com/pgopr/pgopr) and that a PostgreSQL primary has been provided.

See [install pgopr](./01_install_operator.md) and [provision](./02_provision.md) for more detail.

### Accessing database

1. Find the PostgreSQL pod name:

```bash
kubectl get pods
```

Output:

```bash
NAME                          READY   STATUS    RESTARTS   AGE
postgresql-66cfcfc489-s8w2j   1/1     Running   0          22s
```

Look for a pod name starting with 'postgresql-'

2. Use the pod name for port-forwarding:

```bash
kubectl port-forward postgresql-YOURPODNAME 5432:5432
```

Output:

```bash
Forwarding from 127.0.0.1:5432 -> 5432
Forwarding from [::1]:5432 -> 5432
```

3. Connect to database:

   Using PostgreSQL’s built-in terminal-based interface:

```bash
psql -h localhost -p 5432 -U myuser --password mydb
```

Using `mypass` as the password.

Output:

```bash
psql (13.20, server 13.7 (Debian 13.7-1.pgdg110+1))
Type "help" for help.

mydb=#
```
