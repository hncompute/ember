#!/usr/bin/env bash
# Download the prebuilt x86_64 kernel used by the Firecracker quickstart.
set -euo pipefail

url='https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/kernels/vmlinux.bin'
out_dir="${1:-"$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/assets"}"
mkdir -p "$out_dir"

curl --fail --location --retry 3 --output "$out_dir/vmlinux.bin" "$url"
printf 'Downloaded %s/vmlinux.bin\n' "$out_dir"
