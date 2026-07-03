FROM golang:1.25-bookworm AS build

WORKDIR /src

COPY go.mod go.sum ./
RUN go mod download

COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build \
    -trimpath \
    -ldflags="-s -w" \
    -o /out/radishlex-sync-server \
    ./cmd/radishlex-sync-server

FROM debian:bookworm-slim AS runtime

RUN mkdir -p /var/lib/radishlex/sync-server/objects \
    && chown -R 10001:10001 /var/lib/radishlex

COPY --from=build /out/radishlex-sync-server /usr/local/bin/radishlex-sync-server

USER 10001:10001
EXPOSE 7319

ENV RADISHLEX_SYNC_LISTEN=0.0.0.0:7319
ENV RADISHLEX_SYNC_METADATA_PATH=/var/lib/radishlex/sync-server/sync-server.sqlite
ENV RADISHLEX_SYNC_BLOB_DIR=/var/lib/radishlex/sync-server/objects
ENV RADISHLEX_SYNC_MAX_OBJECT_BYTES=16777216
ENV RADISHLEX_SYNC_RECOVERY_READS_PER_HOUR=12

ENTRYPOINT ["/usr/local/bin/radishlex-sync-server"]
