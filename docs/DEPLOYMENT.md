# Production Deployment Plan

Status: **proposed — not yet production-ready.** This document is the plan and
acceptance checklist for running the crawler and API as containers.

## Goal

Run the crawler and API from containers on a single Linux host:

- `halal-api` — long-running, read-only HTTP service.
- `halal-crawler` — one-shot batch job, scheduled weekly.
- PostgreSQL — container with a persistent volume, or a managed external
  service.
- A TLS reverse proxy in front of the API.

The application is already container-friendly: separate binaries, rustls (no
OpenSSL), a read-only API database seam, and `/health`. The gaps are below.

## Target architecture

```
Internet
   │
   ▼
Caddy / nginx (TLS, rate limit)
   │
   ▼
halal-api container ──────────► PostgreSQL (persistent volume)
                                      ▲
systemd timer ──► podman run halal-crawler all ──► Portal
                                      │
                              one-shot migrate job
```

One multi-stage `Dockerfile` builds two runtime targets (`api`, `crawler`) from
the same release binary set. The crawler and API use separate database roles.

## Readiness checklist

### P0 — blockers

- [ ] **Failure visibility.** `main` exits 0 when `RunReport.failures` is
      non-empty. Return non-zero so systemd/containers mark the run failed.
- [ ] **HTTP status handling.** `Portal::retrying` returns non-5xx error bodies
      as listings. Retry 408/429/5xx and surface other 4xx instead of parsing an
      error page as empty data.
- [ ] **Schema bootstrap.** Add a `halal-crawler migrate` job (or a dedicated
      migration binary) and run it before the API and crawler. The API uses
      `db::connect`; the crawler should stop running DDL on every weekly start.
- [ ] **Database roles.** `halal_crawler` owns DDL/read-write;
      `halal_api` has `SELECT` only. Use separate `DATABASE_URL`s.
- [ ] **Product job duration.** A weekly `all` run may exceed seven days at
      product-job volume. The target is weekly `all`; if it does, add
      checkpointing (F1) or split the cadence.
- [ ] **Production images.** Multi-stage build, non-root user, CA certificates,
      `.dockerignore`, pinned base images, image labels.
- [ ] **Production runtime.** Compose/systemd stack with a Postgres volume,
      healthchecks, restart policies, and no published database port.
- [ ] **API exposure.** Set `API_BIND=0.0.0.0:8080` in the container, publish
      only to the reverse proxy, set an explicit CORS allowlist.
- [ ] **Secrets and configuration.** Environment files mode 600; no default
      credentials in production; DB TLS (`sslmode`) if Postgres is remote.
- [ ] **Backups.** Postgres volume plus a `pg_dump` schedule, retention, and a
      tested restore.

### P1 — hardening

- [ ] CI with a PostgreSQL service: `cargo fmt --check`, `cargo clippy`,
      `cargo test`, OpenAPI lint.
- [ ] Image build/scan/push workflow; immutable tags and rollback tag.
- [ ] Resource limits for API, crawler, and Postgres; size DB pools for two
      processes.
- [ ] Alert on crawler non-zero exit; persist per-target failure counts.
- [ ] Structured crawler logging or tracing (API already uses tracing).
- [ ] API rate limiting and request timeouts at the proxy.
- [ ] Image update procedure and base-image refresh cadence.
- [ ] Optional: serve the OpenAPI spec from the API.

## 1. Application changes

### Exit status

`main` should print the existing summary and then exit non-zero when
`RunReport.failures` is non-empty. A scheduled job must not look successful
after a failed target.

### HTTP status handling

`Portal::retrying` should:

- retry transport failures, 408, 429, and 5xx with backoff;
- treat other non-2xx responses as errors, not as listing HTML;
- keep the error chain in `types::error_chain`.

### Migration command

Add a `migrate` job to the crawler binary:

```bash
halal-crawler migrate
```

It calls the schema bootstrap (`db::init` today) and exits without contacting
the portal. The API and crawler then use `db::connect` in production. The
longer-term direction is versioned `sqlx` migrations; the current
`migrations/0001` is a manual migration for pre-split databases.

On a fresh database, run `halal-crawler migrate` before the first API or crawl
job. The crawl jobs switch from `db::init` to `db::connect`, so the README quick
start needs a migrate step; tests continue to use `db::init`.

### Database roles

Add `deploy/sql/roles.sql`:

- `halal_crawler`: schema owner, `CREATE`/`ALTER` and read/write.
- `halal_api`: `SELECT` on `companies`, `products`, `product_categories`,
  `scrap_log`.

The crawler's `DATABASE_URL` and the API's `DATABASE_URL` differ.

## 2. Images

`Dockerfile` (multi-stage, two targets):

```dockerfile
# syntax=docker/dockerfile:1
FROM rust:1.88 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked --bin halal-crawler --bin halal-api

FROM debian:bookworm-slim AS runtime-base
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN groupadd --gid 10001 halal \
    && useradd --uid 10001 --gid 10001 --create-home --shell /usr/sbin/nologin halal

FROM runtime-base AS crawler
COPY --from=builder /app/target/release/halal-crawler /usr/local/bin/
USER 10001
ENTRYPOINT ["halal-crawler"]
CMD ["all"]

FROM runtime-base AS api
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/halal-api /usr/local/bin/
USER 10001
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8080/health || exit 1
ENTRYPOINT ["halal-api"]
```

Build:

```bash
podman build --target crawler -t localhost/halal-crawler:latest .
podman build --target api -t localhost/halal-api:latest .
```

Add `.dockerignore` for `target`, `.git`, `.env*`, `docs`, and `tests`.
Pin base-image tags; avoid building with a moving `latest` toolchain.

## 3. Runtime

`compose.prod.yaml` (long-running services):

- `postgres`: named volume, healthcheck, no host port, resource limits.
- `migrate`: one-shot crawler image with `migrate`, waits for Postgres.
- `api`: `depends_on` Postgres healthy and migrate complete, restart policy,
  read-only filesystem, `cap_drop: [ALL]`, `no-new-privileges`, published only
  to the proxy.
- `proxy`: TLS/rate limiting (Caddy or the existing reverse proxy).

Use a shared Podman network so the timer and compose services resolve the same
Postgres hostname:

```bash
podman network create halal
```

The production `DATABASE_URL` then uses `postgres` as the host, not `localhost`.

Weekly job via systemd (one-shot Podman container):

```ini
[Unit]
Description=Halal crawler weekly run
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
RuntimeDirectory=halal-crawler
ExecStart=/usr/bin/flock -n %t/halal-crawler/crawl.lock \
  /usr/bin/podman run --rm --replace --name halal-crawler \
  --env-file /etc/halal-crawler.env \
  --network halal \
  --security-opt no-new-privileges --cap-drop all --read-only --tmpfs /tmp \
  --pull=never \
  localhost/halal-crawler:latest all
TimeoutStartSec=infinity
StandardOutput=journal
StandardError=journal
```

```ini
[Unit]
Description=Weekly Halal crawler timer

[Timer]
OnCalendar=Mon *-*-* 02:00:00
Persistent=true
RandomizedDelaySec=15m
Unit=halal-crawler.service

[Install]
WantedBy=timers.target
```

Rootless Podman uses a user timer and `loginctl enable-linger`; rootful Podman
uses a system unit. The container runs non-root in both cases. For a user timer,
use `%h/.config/halal-crawler.env` for `--env-file`; `%t` resolves to the user's
runtime directory, so the lock path above works unchanged.

## 4. Configuration and secrets

Production environment files (`/etc/halal-crawler.env`,
`/etc/halal-api.env`, mode 600):

- `DATABASE_URL` per role; add `sslmode=require` for remote Postgres.
- `API_BIND=0.0.0.0:8080`.
- `API_CORS_ORIGINS` — explicit origins, not `*`.
- `HALAL_MAX_PAGES=0` or unset for a full crawl; never copy the example's `1`
  into production.
- `RUST_LOG` for the API.
- No credentials in images, Compose files, or shell history.

## 5. Scheduling

- One weekly `all` job: companies first, then products.
- `Persistent=true` catches missed runs after downtime.
- `flock` prevents overlapping runs.
- `TimeoutStartSec=infinity`; do not rely on the systemd default 90s timeout.
- Journald is the log sink; alert on non-zero exit.
- If the product job does not fit in a week, switch to checkpointing (F1) or
  run companies weekly and products on a longer cadence.

## 6. Backups and observability

- Back up Postgres with `pg_dump` on a timer, retain multiple copies, and
  test a restore.
- If Postgres is managed, use its backups and point-in-time recovery.
- Monitor: crawler exit status, `scrap_log` failure counts, API `/health`,
  container restarts, disk usage for the Postgres volume.
- Persist per-target failure/capped counts (F9) so a “successful” run with
  skipped letters is visible.

## 7. CI/CD

- **Test:** PostgreSQL service; `cargo fmt --check`, `cargo clippy`,
  `cargo test`, `npx @redocly/cli lint docs/api/openapi.yaml`.
- **Images:** build `api` and `crawler`; smoke the API against Postgres; scan
  with Trivy; push to the registry with a commit/tag label.
- **Deploy:** pull a specific tag; run `migrate`; restart the API; run the
  weekly timer. Keep the previous image tag for rollback.

## 8. Open decisions

1. **Postgres:** self-hosted container with a volume, or managed/external?
2. **TLS/proxy:** existing proxy/domain, or include Caddy?
3. **Podman:** rootless or rootful?
4. **Cadence:** the chosen shape is one weekly `all`; if it exceeds a week,
   checkpoint (F1) or split companies weekly and products on a longer cadence.
5. **Registry:** GHCR, Docker Hub, or a private registry?

## 9. Implementation sequence

1. Failure visibility (exit status + 4xx/429).
2. `migrate` command and database roles.
3. Dockerfile, `.dockerignore`, and image smoke tests.
4. Production compose/systemd, environment examples, and the weekly timer.
5. TLS proxy, secrets, backups, and resource limits.
6. CI test/image workflow.
7. Capped smoke deploy, then enable the weekly timer.

## 10. Acceptance criteria

- Both images build from a clean checkout with `--locked`.
- A fresh database is created by `halal-crawler migrate`; the API starts and
  `/health` returns 200.
- The API runs as non-root, with a read-only filesystem, no added capabilities,
  and a read-only database role.
- A capped crawler run succeeds; a forced target failure exits non-zero.
- The weekly timer fires, survives a missed run, and does not overlap itself.
- A Postgres backup restores into a clean database.
- The OpenAPI spec and tests pass in CI; the image is scanned and tagged.
