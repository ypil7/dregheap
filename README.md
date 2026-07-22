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
