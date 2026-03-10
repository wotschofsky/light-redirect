# light-redirect

A super-lightweight HTTP redirect server written in Rust.

[![Docker Image Size](https://img.shields.io/docker/image-size/ghcr.io/wotschofsky/light-redirect/latest?label=image%20size)](https://github.com/wotschofsky/light-redirect/pkgs/container/light-redirect)
[![GitHub Release](https://img.shields.io/github/v/release/wotschofsky/light-redirect)](https://github.com/wotschofsky/light-redirect/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## What is this?

`light-redirect` is a minimal HTTP redirect server — a lightweight alternative to [`schmunk42/nginx-redirect`](https://hub.docker.com/r/schmunk42/nginx-redirect) without the nginx overhead.

- Single statically linked binary (no libc, no shell)
- `scratch`-based Docker image — under **5 MB**
- Zero configuration bloat: only 3 environment variables

## Quick start

```sh
docker run -e SERVER_REDIRECT=www.example.com ghcr.io/wotschofsky/light-redirect
```

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SERVER_REDIRECT` | Yes | — | Target host to redirect all traffic to |
| `SERVER_REDIRECT_PATH` | No | _(original request URI)_ | Fixed path (must start with `/`) to use in every redirect |
| `SERVER_REDIRECT_CODE` | No | `301` | HTTP redirect code (`301`, `302`, `303`, `307`, `308`) |
| `SERVER_PORT` | No | `80` | Port to listen on |
| `SERVER_HEALTH_PATH` | No | _(disabled)_ | Path that returns `200 OK` for health checks (e.g. `/healthz`) |

## Behaviour

- All requests redirect to `https://{SERVER_REDIRECT}{path}`
- Scheme is always `https` (non-configurable)
- Default port is `80` (configurable via `SERVER_PORT`; TLS termination expected upstream)
- Access logs are written to `stdout`, errors to `stderr`
- Graceful shutdown on `SIGINT`/`Ctrl+C`

## Docker Compose example

```yaml
services:
  redirect:
    image: ghcr.io/wotschofsky/light-redirect:latest
    environment:
      SERVER_REDIRECT: www.example.com
      SERVER_REDIRECT_CODE: "301"
    ports:
      - "80:80"
```

## Building from source

**Prerequisites:** Rust toolchain, Docker with Buildx

```sh
# Build the binary
cargo build --release

# Build the Docker image
docker build -t light-redirect .
```

## Comparison with nginx-redirect

| Attribute | nginx-redirect | light-redirect |
|-----------|---------------|----------------|
| Base image | `nginx:alpine` | `scratch` |
| Image size | ~25 MB | <5 MB |
| Runtime | nginx + bash | single binary |
| Config variables | 9 | 3 |

## License

MIT
