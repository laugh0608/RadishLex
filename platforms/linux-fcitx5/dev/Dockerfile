FROM debian:13-slim@sha256:020c0d20b9880058cbe785a9db107156c3c75c2ac944a6aa7ab59f2add76a7bd

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get -o Acquire::Retries=3 update \
    && apt-get -o Dpkg::Use-Pty=0 install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        cargo \
        cmake \
        fcitx5-modules-dev \
        file \
        libfcitx5core-dev \
        libfcitx5utils-dev \
        librime-dev \
        ninja-build \
        pkg-config \
        ripgrep \
        rustc \
    && rm -rf /var/lib/apt/lists/*

ENV CARGO_HOME=/tmp/radishlex-cargo-home \
    CARGO_TARGET_DIR=/tmp/radishlex-target

WORKDIR /workspace/RadishLex
