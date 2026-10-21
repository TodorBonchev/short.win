# shortwin-indexer image (docs/design/INDEXER.md §12.1). Build context: the repository root
# (the indexer links programs/shortwin and crates/* by path).
FROM rust:1.98.1-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e AS build
WORKDIR /src
COPY . .
RUN cargo build --manifest-path services/Cargo.toml -p shortwin-indexer --release --locked

FROM debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 shortwin
COPY --from=build /src/services/target/release/shortwin-indexer /usr/local/bin/shortwin-indexer
USER shortwin
ENTRYPOINT ["shortwin-indexer"]
