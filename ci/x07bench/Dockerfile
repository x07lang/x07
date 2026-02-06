FROM rust:1

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        build-essential \
        clang \
        git \
        patch \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . /src
RUN cargo build --release -p x07 \
    && install -m 0755 target/release/x07 /usr/local/bin/x07

WORKDIR /work
ENTRYPOINT ["/usr/local/bin/x07"]
