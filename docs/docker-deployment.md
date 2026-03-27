# Docker Deployment

The Omnivore Dashboard is a single Rust binary with an embedded SQLite database — ideal for lightweight, always-on Docker deployments.

## Quick Start

```sh
cd dashboard
docker build -t omnivore-dashboard .
docker run -d \
  --name omnivore \
  -p 3000:3000 \
  -v omnivore-data:/data \
  omnivore-dashboard
```

The dashboard is now running at `http://localhost:3000`.

## Configuration

All configuration is via environment variables:

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | `sqlite:/data/omnivore.db?mode=rwc` | SQLite database path |
| `BIND_ADDR` | `0.0.0.0:3000` | Listen address and port |
| `RUST_LOG` | `info` | Log level (`debug`, `info`, `warn`, `error`) |
| `GITHUB_TOKEN` | *(none)* | GitHub token for PR comments and source fetching |
| `OMNIVORE_DASHBOARD_URL` | *(none)* | Public URL for PR comment links (e.g., `https://omnivore.example.com`) |
| `OMNIVORE_RETENTION_FULL` | `30` | Number of full snapshots to retain per project+target |
| `OMNIVORE_RETENTION_SUMMARY` | `60` | Number of summary-only snapshots to retain for trend charts |
| `GITHUB_CLIENT_ID` | *(none)* | GitHub OAuth App client ID (enables login) |
| `GITHUB_CLIENT_SECRET` | *(none)* | GitHub OAuth App client secret |
| `OMNIVORE_STATIC_DIR` | *(compile-time)* | Path to static assets directory (set automatically in Docker) |
| `OMNIVORE_GITHUB_ORG` | *(none)* | GitHub org for admin resolution (org owners = dashboard admins) |

Pass env vars with `-e`:

```sh
docker run -d \
  --name omnivore \
  -p 3000:3000 \
  -v omnivore-data:/data \
  -e GITHUB_TOKEN=ghp_xxx \
  -e OMNIVORE_DASHBOARD_URL=http://192.168.1.100:3000 \
  omnivore-dashboard
```

Or use an env file:

```sh
# omnivore.env
GITHUB_TOKEN=ghp_xxx
OMNIVORE_DASHBOARD_URL=http://192.168.1.100:3000

docker run -d \
  --name omnivore \
  -p 3000:3000 \
  -v omnivore-data:/data \
  --env-file omnivore.env \
  omnivore-dashboard
```

## Data Persistence

The SQLite database lives at `/data/omnivore.db` inside the container. **Always mount a volume** to `/data` so data survives container restarts:

```sh
# Named volume (Docker manages the location)
-v omnivore-data:/data

# Bind mount (you choose the host path)
-v /path/on/host:/data
```

## Docker Compose

```yaml
services:
  omnivore:
    build: ./dashboard
    ports:
      - "3000:3000"
    volumes:
      - omnivore-data:/data
    environment:
      - RUST_LOG=info
    restart: unless-stopped

volumes:
  omnivore-data:
```

## QNAP NAS Deployment

The dashboard runs comfortably on QNAP NAS devices via Container Station. The Rust binary is small, SQLite needs minimal RAM, and coverage uploads are infrequent.

### Option 1: Container Station UI

1. Copy the `dashboard/` directory to your QNAP (or clone the repo)
2. Open **Container Station** > **Create** > **Docker Compose**
3. Point to the `docker-compose.yml` or paste the compose config above
4. Set a shared folder as the data volume (e.g., `/share/Container/omnivore:/data`)
5. Click **Create**

### Option 2: SSH + Docker CLI

```sh
ssh admin@<qnap-ip>

# Build (or pull a pre-built image if published)
cd /share/repos/omnivore/dashboard
docker build -t omnivore-dashboard .

# Run with a bind mount to a shared folder
docker run -d \
  --name omnivore \
  -p 3000:3000 \
  -v /share/Container/omnivore-data:/data \
  --restart unless-stopped \
  omnivore-dashboard
```

### Pointing CI / Test Rigs at the NAS

Update your upload targets to use the NAS IP:

```sh
# Gradle plugin (gradle.properties or env var)
omnivore.dashboardUrl=http://<qnap-ip>:3000

# curl (Rust, Go, Python test rigs)
curl -X POST "http://<qnap-ip>:3000/api/v1/ingest/coverage?format=llvm-cov&project_id=my-project" \
  -H "X-API-Key: omni_xxx" \
  --data-binary @coverage.json

# GitHub Actions
env:
  OMNIVORE_URL: http://<qnap-ip>:3000
```

### Resource Requirements

| Resource | Requirement |
|---|---|
| CPU | Minimal — idle most of the time |
| RAM | ~20-30 MB resident |
| Disk | Binary ~10 MB + DB grows with snapshots |
| Network | Port 3000 (configurable via `BIND_ADDR`) |

## Updating

```sh
# Rebuild with latest code
cd dashboard
docker build -t omnivore-dashboard .

# Restart container (data volume persists)
docker stop omnivore && docker rm omnivore
docker run -d \
  --name omnivore \
  -p 3000:3000 \
  -v omnivore-data:/data \
  --restart unless-stopped \
  omnivore-dashboard
```

## Troubleshooting

**Container exits immediately**: Check logs with `docker logs omnivore`. Common cause: volume mount permissions.

**Database locked errors**: SQLite supports one writer at a time. This is fine for normal usage but could be an issue under heavy concurrent ingestion. The connection pool (max 5) handles this gracefully.

**Can't reach from other machines**: Ensure port 3000 is open in your firewall/NAS security settings and you're using `0.0.0.0` (not `127.0.0.1`) as the bind address.
