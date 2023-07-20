#!/usr/bin/env bash

set -e

git_hash=$(git rev-parse --short HEAD)
image_name="ghcr.io/ragtag-archive/archivebot"

for target in aarch64-unknown-linux-musl x86_64-unknown-linux-musl; do
    echo "[*] Building for $target"

    rustup target add $target
    cross build --release --locked --target $target

    docker_platform=""
    arch_prefix=""
    arch_tag=""
    if [ $target = "aarch64-unknown-linux-musl" ]; then
        docker_platform="linux/arm64"
        arch_prefix="arm64v8/"
        arch_tag="arm64"
    elif [ $target = "x86_64-unknown-linux-musl" ]; then
        docker_platform="linux/amd64"
        arch_prefix=""
        arch_tag="amd64"
    fi

    echo "[*] Building docker image for $target"
    cp target/$target/release/archivebot archivebot
    docker build \
        -f Dockerfile.cross \
        -t $image_name:$arch_tag \
        --platform $docker_platform \
        --build-arg ARCH_PREFIX=$arch_prefix \
        .
    rm archivebot

    echo "[*] Pushing docker image for $target"
    docker push $image_name:$arch_tag
done

# Create manifest
echo "[*] Creating manifest"
export DOCKER_CLI_EXPERIMENTAL=enabled
docker manifest create $image_name:$git_hash \
    $image_name:aarch64 $image_name:amd64
docker manifest create $image_name:latest \
    $image_name:aarch64 $image_name:amd64

echo "[*] Pushing manifest"
docker manifest push $image_name:$git_hash
docker manifest push $image_name:latest

echo "[*] Done"