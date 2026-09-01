#!/usr/bin/env bash
# Download static BusyBox and build a minimal x86_64 initramfs for ember.
set -euo pipefail

alpine_index='https://dl-cdn.alpinelinux.org/alpine/latest-stable/main/x86_64/APKINDEX.tar.gz'
out_dir="${1:-"$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/assets"}"
out_dir="$(mkdir -p "$out_dir" && cd "$out_dir" && pwd)"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

for command in curl tar find; do
    command -v "$command" >/dev/null || {
        echo "Required command not found: $command" >&2
        exit 1
    }
done

busybox_version="$(curl --fail --location --retry 3 "$alpine_index" \
    | tar -xzO APKINDEX \
    | awk '/^P:busybox-static$/ { found = 1; next } found && /^V:/ { sub(/^V:/, ""); print; found = 0 }')"
: "${busybox_version:?Could not determine the Alpine busybox-static version}"

busybox_apk="$work_dir/busybox-static.apk"
curl --fail --location --retry 3 --output "$busybox_apk" \
    "https://dl-cdn.alpinelinux.org/alpine/latest-stable/main/x86_64/busybox-static-${busybox_version}.apk"

tree="$work_dir/root"
mkdir -p "$tree"/{bin,dev,proc,sys,tmp}
tar -xzf "$busybox_apk" -C "$tree" bin/busybox.static
mv "$tree/bin/busybox.static" "$tree/bin/busybox"
chmod 0755 "$tree/bin/busybox"

cat >"$tree/init" <<'EOF'
#!/bin/busybox sh
/bin/busybox mount -t devtmpfs devtmpfs /dev
/bin/busybox mount -t proc proc /proc
/bin/busybox mount -t sysfs sysfs /sys
exec /bin/busybox sh
EOF
chmod 0755 "$tree/init"

(
    cd "$tree"
    find . -print | ./bin/busybox cpio -o -H newc >"$out_dir/initramfs.cpio"
)
printf 'Built %s/initramfs.cpio\n' "$out_dir"
