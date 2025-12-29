FROM rust:1-slim-bookworm AS builder

WORKDIR /usr/src/app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN set -ex; \
    mkdir src; \
    echo 'fn main() {}' > src/main.rs; \
    echo 'fn lib() {}' > src/lib.rs; \
    cargo build --release; \
    rm -rf src;

# Build project
COPY . .
RUN set -ex; \
    touch src/main.rs src/lib.rs; \
    cargo build --release;

FROM debian:bookworm-slim

ENV PATH="/root/.cache/archivebot:$PATH"

RUN set -ex; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates libexpat1 curl; \
    rm -rf /var/lib/apt/lists/*; \
    update-ca-certificates;

COPY --from=builder /usr/src/app/target/release/archivebot /usr/local/bin/archivebot
COPY --from=denoland/deno:bin-2.6.3 /deno /usr/local/bin/deno

CMD /usr/local/bin/archivebot
