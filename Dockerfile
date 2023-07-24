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
COPY --from=builder /usr/src/app/target/release/archivebot /usr/local/bin/archivebot

CMD /usr/local/bin/archivebot