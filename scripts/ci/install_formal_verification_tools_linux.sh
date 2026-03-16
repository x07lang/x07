#!/usr/bin/env bash
set -euo pipefail

cbmc_version="${X07_CI_CBMC_VERSION:-6.8.0}"
z3_version="${X07_CI_Z3_VERSION:-4.16.0}"
arch="$(uname -m)"

case "$arch" in
  x86_64)
    cbmc_deb="ubuntu-24.04-cbmc-${cbmc_version}-Linux.deb"
    z3_dir="z3-${z3_version}-x64-glibc-2.39"
    ;;
  aarch64 | arm64)
    cbmc_deb="ubuntu-24.04-arm64-cbmc-${cbmc_version}-Linux.deb"
    z3_dir="z3-${z3_version}-arm64-glibc-2.38"
    ;;
  *)
    echo "error: unsupported architecture for formal verification tool install: $arch" >&2
    exit 1
    ;;
esac

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: missing required tool: $1" >&2
    exit 1
  }
}

need curl
need unzip

if [[ "$(id -u)" == "0" ]]; then
  sudo_cmd=()
else
  sudo_cmd=(sudo)
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

cbmc_url="https://github.com/diffblue/cbmc/releases/download/cbmc-${cbmc_version}/${cbmc_deb}"
z3_url="https://github.com/Z3Prover/z3/releases/download/z3-${z3_version}/${z3_dir}.zip"
z3_root="/usr/local/libexec/${z3_dir}"

curl -fsSL "$cbmc_url" -o "$tmp_dir/cbmc.deb"
"${sudo_cmd[@]}" apt-get install -y "$tmp_dir/cbmc.deb"

curl -fsSL "$z3_url" -o "$tmp_dir/z3.zip"
"${sudo_cmd[@]}" rm -rf "$z3_root"
"${sudo_cmd[@]}" mkdir -p /usr/local/libexec
"${sudo_cmd[@]}" unzip -q "$tmp_dir/z3.zip" -d /usr/local/libexec
"${sudo_cmd[@]}" ln -sf "${z3_root}/bin/z3" /usr/local/bin/z3

cbmc --version
z3 -version
