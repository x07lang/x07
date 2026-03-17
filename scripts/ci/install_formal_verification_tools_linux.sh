#!/usr/bin/env bash
set -euo pipefail

cbmc_version="${X07_CI_CBMC_VERSION:-6.8.0}"
z3_version="${X07_CI_Z3_VERSION:-4.16.0}"
arch="$(uname -m)"
os_id=""
os_version_id=""

if [[ -r /etc/os-release ]]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  os_id="${ID:-}"
  os_version_id="${VERSION_ID:-}"
fi

cbmc_deb=""
z3_asset_url=""
z3_root=""
z3_binary_path=""

case "$arch" in
  x86_64)
    case "${os_id}:${os_version_id}" in
      ubuntu:22.04)
        cbmc_deb="ubuntu-22.04-cbmc-${cbmc_version}-Linux.deb"
        z3_wheel="z3_solver-${z3_version}.0-py3-none-manylinux_2_27_x86_64.whl"
        z3_asset_url="https://files.pythonhosted.org/packages/py3/z/z3-solver/${z3_wheel}"
        z3_root="/usr/local/libexec/${z3_wheel%.whl}"
        z3_binary_path="${z3_root}/z3_solver-${z3_version}.0.data/data/bin/z3"
        ;;
      *)
        z3_dir="z3-${z3_version}-x64-glibc-2.39"
        cbmc_deb="ubuntu-24.04-cbmc-${cbmc_version}-Linux.deb"
        z3_asset_url="https://github.com/Z3Prover/z3/releases/download/z3-${z3_version}/${z3_dir}.zip"
        z3_root="/usr/local/libexec/${z3_dir}"
        z3_binary_path="${z3_root}/bin/z3"
        ;;
    esac
    ;;
  aarch64 | arm64)
    if [[ "${os_id}:${os_version_id}" == "ubuntu:22.04" ]]; then
      echo "error: Ubuntu 22.04 ${arch} does not have supported upstream formal-verification tool assets" >&2
      exit 1
    fi
    cbmc_deb="ubuntu-24.04-arm64-cbmc-${cbmc_version}-Linux.deb"
    z3_dir="z3-${z3_version}-arm64-glibc-2.38"
    z3_asset_url="https://github.com/Z3Prover/z3/releases/download/z3-${z3_version}/${z3_dir}.zip"
    z3_root="/usr/local/libexec/${z3_dir}"
    z3_binary_path="${z3_root}/bin/z3"
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

curl -fsSL "$cbmc_url" -o "$tmp_dir/cbmc.deb"
"${sudo_cmd[@]}" apt-get install -y "$tmp_dir/cbmc.deb"

z3_download_path="${tmp_dir}/$(basename "$z3_asset_url")"
curl -fsSL "$z3_asset_url" -o "$z3_download_path"
"${sudo_cmd[@]}" rm -rf "$z3_root"
"${sudo_cmd[@]}" mkdir -p /usr/local/libexec
case "$z3_download_path" in
  *.whl | *.zip)
    "${sudo_cmd[@]}" unzip -q "$z3_download_path" -d "$z3_root"
    ;;
  *)
    echo "error: unsupported z3 artifact format: $z3_download_path" >&2
    exit 1
    ;;
esac
"${sudo_cmd[@]}" ln -sf "$z3_binary_path" /usr/local/bin/z3

cbmc --version
z3 -version
