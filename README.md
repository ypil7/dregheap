# Dreg Heap

Very much in-progress

## Docker

Create or edit `.env.local` with the `DREG_` settings the server should receive,
then run:

```sh
docker-compose up --build
```

`docker-compose.yml` loads `.env.local` with `env_file`, so those variables are
injected into the container environment.

## Configuration

- `DREG_MAX_CACHE_WEIGHT`: optional maximum cache weight in bytes. Each entry
  counts key bytes, value bytes, and a fixed 64 byte overhead. When unset, the
  cache is unbounded and the server logs a warning on startup.
