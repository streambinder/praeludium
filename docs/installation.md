# Installation

## Container image

The image is built for `linux/arm64` and published on the GitHub
Container Registry:

```bash
docker pull ghcr.io/streambinder/praeludium:latest
```

The image name takes a unique value per commit beside `latest`:
`ghcr.io/streambinder/praeludium:<commit sha>`. A release tag
publishes the `release`, version, and commit tags instead.

Run the application with a volume for the data:

```bash
docker run -d --name praeludium -p 127.0.0.1:8080:8080 \
  -v praeludium-data:/data \
  ghcr.io/streambinder/praeludium:latest
```

Then open <http://localhost:8080>.

The application has no authentication of its own. In production,
expose the application only behind a reverse proxy. The proxy must
enforce HTTP Basic Auth. The proxy must leave the `/share/` paths
open: share links are public by design.

## Configuration

| Variable              | Default | Description               |
| --------------------- | ------- | ------------------------- |
| `PRAELUDIUM_DATA_DIR` | `/data` | SQLite database directory |
| `PORT`                | `8080`  | HTTP port                 |

The database file is `praeludium.db` in the data directory. Back
up the whole directory to back up the application.

## Build from source

Præludium is one Go module with no C dependencies. The SQLite
driver is pure Go.

```bash
git clone https://github.com/streambinder/praeludium.git
cd praeludium
go build ./cmd/praeludium
```

## Local development

Run the application from the repository root (Go 1.26 or later):

```bash
PRAELUDIUM_DATA_DIR=/tmp/praeludium PORT=8080 go run ./cmd/praeludium
```

Run the tests with `go test ./...`. The unit test coverage gate
is 90 percent.
