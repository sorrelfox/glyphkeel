# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# CI image: the current Fedora release. Built and run by
# `cargo xtask ci --distro fedora`.
FROM registry.fedoraproject.org/fedora:45

ARG RUST_TOOLCHAIN
ARG NODE_VERSION
ARG PNPM_VERSION

ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    COREPACK_HOME=/usr/local/corepack \
    COREPACK_ENABLE_DOWNLOAD_PROMPT=0 \
    PNPM_HOME=/usr/local/pnpm \
    CI=1
ENV PATH=/usr/local/cargo/bin:$PATH

# Build tools and the desktop app's system libraries (WebKitGTK 4.1, GTK 3,
# libsoup 3), plus Xvfb for headless UI runs.
RUN dnf install -y --setopt=install_weak_deps=False \
      gcc gcc-c++ make curl file git pkgconf-pkg-config xz tar \
      webkit2gtk4.1-devel gtk3-devel libsoup3-devel librsvg2-devel \
      xorg-x11-server-Xvfb xorg-x11-xauth \
 && dnf clean all

COPY install-toolchains.sh /usr/local/bin/install-toolchains.sh
RUN install-toolchains.sh

WORKDIR /work
