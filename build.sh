#!/bin/sh

set -eu

buildDir=$(pwd)/build
rootfsDir=${buildDir}/rootfs
builder="/build-initramfs-in-ctr.sh"

if command -v docker >/dev/null; then
  ctrEngine=docker
elif command -v podman >/dev/null; then
  ctrEngine=podman
else
  echo "ERROR: Podman or Docker is required"
  exit 127
fi

mkdir -p $buildDir

$ctrEngine run \
  -it --rm \
  -v${buildDir}:/build \
  -v$(pwd)/container:/container:ro \
  -v$(pwd)/container/${builder}:${builder}:ro \
  -v$(pwd)/guest:/guest:ro \
  alpine:latest \
  ${builder} $@
