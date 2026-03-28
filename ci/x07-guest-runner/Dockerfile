FROM rust:1-bookworm AS builder

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        clang \
        libcurl4-openssl-dev \
        liblzma-dev \
        libsqlite3-dev \
        libssl-dev \
        pkg-config \
        zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . /src

RUN cargo build --release -p x07 -p x07-os-runner -p x07-guestd \
    && ./scripts/build_os_helpers.sh \
    && ./scripts/build_ext_db_mysql.sh \
    && ./scripts/build_ext_db_pg.sh \
    && ./scripts/build_ext_db_redis.sh \
    && ./scripts/build_ext_db_sqlite.sh \
    && ./scripts/build_ext_fs.sh \
    && ./scripts/build_ext_jsonschema.sh \
    && ./scripts/build_ext_math.sh \
    && ./scripts/build_ext_rand.sh \
    && ./scripts/build_ext_regex.sh \
    && ./scripts/build_ext_stdio.sh \
    && ./scripts/build_ext_stream_xf.sh \
    && ./scripts/build_ext_time.sh \
    && install -m 0755 target/release/x07 /usr/local/bin/x07 \
    && install -m 0755 target/release/x07-os-runner /usr/local/bin/x07-os-runner \
    && install -m 0755 target/release/x07-guestd /usr/local/bin/x07-guestd

FROM debian:bookworm-slim

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        clang \
        curl \
        libcurl4 \
        liblzma5 \
        libsqlite3-0 \
        libssl3 \
        nftables \
        pkg-config \
        zlib1g \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/x07 /usr/local/bin/x07
COPY --from=builder /usr/local/bin/x07-os-runner /usr/local/bin/x07-os-runner
COPY --from=builder /usr/local/bin/x07-guestd /usr/local/bin/x07-guestd

WORKDIR /opt/x07
ENV X07_WORKSPACE_ROOT=/opt/x07
COPY --from=builder /src/deps/x07/ ./deps/x07/
COPY stdlib/ ./stdlib/
COPY stdlib.lock ./stdlib.lock
COPY stdlib.os.lock ./stdlib.os.lock
