#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

usage() {
  cat <<'EOF'
Usage:
  scripts/build_vz_guest_bundle.sh --image <oci-ref> --out <dir> [--kernel-image <oci-ref>] [--extra-mib <mib>]

Builds a VZ guest bundle directory containing:
  - manifest.json
  - kernel
  - rootfs.img (ext4, raw)
  - cmdline.txt

Requires:
  - docker
  - mkfs.ext4 or mke2fs (from e2fsprogs)

Example:
  ./scripts/build_guest_runner_image.sh --image x07-guest-runner --tag vm-smoke
  ./scripts/build_vz_guest_bundle.sh --image x07-guest-runner:vm-smoke --out /tmp/x07-guest.bundle
EOF
}

root="$(repo_root)"

image=""
out=""
kernel_image="linuxkit/kernel:6.6.71"
extra_mib="256"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --image)
      image="${2:-}"
      shift 2
      ;;
    --out)
      out="${2:-}"
      shift 2
      ;;
    --kernel-image)
      kernel_image="${2:-}"
      shift 2
      ;;
    --extra-mib)
      extra_mib="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "ERROR: unknown arg: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$image" || -z "$out" ]]; then
  echo "ERROR: --image and --out are required" >&2
  usage >&2
  exit 2
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "ERROR: missing docker" >&2
  exit 2
fi

mkfs_bin=""
if command -v mkfs.ext4 >/dev/null 2>&1; then
  mkfs_bin="mkfs.ext4"
elif command -v mke2fs >/dev/null 2>&1; then
  mkfs_bin="mke2fs"
else
  echo "ERROR: missing mkfs.ext4/mke2fs (install e2fsprogs)" >&2
  exit 2
fi

if [[ -e "$out" ]]; then
  echo "ERROR: output path already exists: $out" >&2
  exit 2
fi

tmp="$(mktemp -d 2>/dev/null || mktemp -d -t x07_vz_bundle)"
cleanup() { rm -rf "$tmp"; }
trap cleanup EXIT

rootfs_dir="$tmp/rootfs"
mkdir -p "$rootfs_dir"

echo "==> export rootfs from $image"
cid="$(docker create "$image" true)"
trap 'docker rm -f "$cid" >/dev/null 2>&1 || true; cleanup' EXIT
docker export "$cid" | tar -C "$rootfs_dir" -xf -
docker rm -f "$cid" >/dev/null 2>&1 || true

echo "==> build ext4 rootfs.img"
root_kib="$(du -sk "$rootfs_dir" | awk '{print $1}')"
extra_kib="$(( extra_mib * 1024 ))"
img_kib="$(( root_kib + extra_kib ))"
img_bytes="$(( img_kib * 1024 ))"

stage="$tmp/out"
mkdir -p "$stage"
rootfs_img="$stage/rootfs.img"
truncate -s "$img_bytes" "$rootfs_img" 2>/dev/null || dd if=/dev/zero of="$rootfs_img" bs=1 count=0 seek="$img_bytes" >/dev/null 2>&1

if [[ "$mkfs_bin" == "mkfs.ext4" ]]; then
  mkfs.ext4 -F -d "$rootfs_dir" "$rootfs_img" >/dev/null
else
  # mke2fs: use -d to populate from directory.
  mke2fs -F -t ext4 -d "$rootfs_dir" "$rootfs_img" >/dev/null
fi

echo "==> extract kernel from $kernel_image"
kernel_path="$stage/kernel"
docker run --rm --entrypoint cat "$kernel_image" /kernel >"$kernel_path"
chmod 0644 "$kernel_path" || true

cmdline_path="$stage/cmdline.txt"
cat >"$cmdline_path" <<'CMD'
root=/dev/vda rw console=hvc0 ip=dhcp init=/usr/local/bin/x07-guestd
CMD

manifest_path="$stage/manifest.json"
cat >"$manifest_path" <<'JSON'
{
  "schema_version": "x07.vz.guest.bundle@0.1.0",
  "linux": {
    "kernel": "kernel",
    "rootfs": "rootfs.img",
    "cmdline": "cmdline.txt"
  }
}
JSON

mkdir -p "$(dirname "$out")"
mv "$stage" "$out"
echo "ok: wrote VZ guest bundle to $out"
