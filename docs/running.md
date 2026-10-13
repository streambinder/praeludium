# Running præludium

## Run with Docker

```sh
docker build -t praeludium .
docker run -d --name praeludium -p 127.0.0.1:8080:8080 \
  -v praeludium-data:/data \
  ghcr.io/streambinder/praeludium:latest
```

Then open <http://localhost:8080>.

The application has no authentication of its own.
In production, expose the application only behind a reverse proxy.
The proxy must enforce HTTP Basic Auth.

## Environment variables

| Variable              | Default | Description               |
| --------------------- | ------- | ------------------------- |
| `PRAELUDIUM_DATA_DIR` | `/data` | SQLite database directory |
| `PORT`                | `8080`  | HTTP port                 |

## Local development

Run `go run .` (Go 1.26 or later).
