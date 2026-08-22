\newpage

# Experimental features

pgopr includes early support for backup, metrics, and monitoring resources.
These features show the intended direction of the project, but they are not yet
production-ready.

The promise is that these resources will become operator-managed parts of the
PostgreSQL cluster:

```text
PgOpr custom resource
  -> PostgreSQL primary and replicas
  -> pgmoneta backup resources
  -> pgexporter metrics resources
  -> optional Grafana/Prometheus monitoring resources
```

The command-line interface should remain thin. Commands such as
`pgopr provision pgmoneta` and `pgopr provision pgexporter` should update the
`PgOpr` resource. The operator then reconciles the Kubernetes child resources.

## pgmoneta

pgmoneta is the planned backup component.

Current behavior:

```bash
pgopr provision pgmoneta
pgopr retire pgmoneta
```

The current implementation creates operator-managed pgmoneta resources when
`spec.pgmoneta` is present. It uses local storage first, with the storage size
coming from `default_pgmoneta_storage` in `~/.pgopr/pgopr.toml` when enabled
through the CLI.

The production direction is:

- pgopr generates or mounts the pgmoneta configuration file
- backup credentials live in Kubernetes Secrets
- local storage uses PV/PVC resources
- remote storage, such as S3, becomes an explicit storage mode
- `PgOpr.status.pgmoneta` reports readiness and failure reasons

Known limitations:

- pgmoneta configuration handling is not complete
- remote storage is not implemented
- PostgreSQL backup user bootstrap is not complete
- restore and backup catalog workflows are not complete
- readiness does not yet prove that a usable backup can be taken

## pgexporter

pgexporter is the planned metrics exporter component.

Current behavior:

```bash
pgopr provision pgexporter
pgopr retire pgexporter
```

The current implementation creates operator-managed pgexporter resources when
`spec.pgexporter` is present.

The production direction is:

- exporter credentials live in Kubernetes Secrets
- PostgreSQL exporter user permissions are created or validated
- exporter configuration is generated or referenced explicitly
- `PgOpr.status.pgexporter` reports readiness and failure reasons
- metrics exposure is deliberate rather than accidental

Known limitations:

- exporter credentials are not production-ready
- PostgreSQL exporter user bootstrap is not complete
- endpoint security is not complete
- readiness does not yet prove PostgreSQL metrics are being collected

## Grafana and Prometheus

Grafana/Prometheus support is the planned monitoring companion for pgexporter.

Current behavior:

```bash
pgopr provision grafana
pgopr retire grafana
```

This feature is experimental and should be treated as a development preview.

The production direction is:

- monitoring is enabled through `PgOpr.spec.pgexporter.monitoring`
- Grafana and Prometheus configuration is explicit
- credentials and persistence are modeled
- status proves the monitoring stack is wired to pgexporter

Known limitations:

- persistence is not modeled
- dashboard and datasource configuration are not stable
- access and credentials are not production-defined
- readiness does not yet prove Prometheus is scraping pgexporter

For the full readiness checklist, see `doc/FEATURE_READINESS.md`.
