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
# libsoup 3), plus Xvfb for headless UI runs and the libraries Playwright's
# Chromium needs (Playwright installs them itself only on Debian and Ubuntu).
RUN dnf install -y --setopt=install_weak_deps=False \
      gcc gcc-c++ make curl file git pkgconf-pkg-config xz tar \
      webkit2gtk4.1-devel gtk3-devel libsoup3-devel librsvg2-devel \
      xorg-x11-server-Xvfb xorg-x11-xauth \
      nss nspr atk at-spi2-atk at-spi2-core cups-libs libdrm libxkbcommon \
      libXcomposite libXdamage libXfixes libXrandr mesa-libgbm pango cairo \
      alsa-lib \
 && dnf clean all

COPY install-toolchains.sh /usr/local/bin/install-toolchains.sh
RUN install-toolchains.sh

# Chromium for the UI's Playwright flows, in its own layer so that a new
# Playwright version doesn't rebuild the toolchains.
ARG PLAYWRIGHT_VERSION
ENV PLAYWRIGHT_BROWSERS_PATH=/usr/local/ms-playwright
COPY install-browsers.sh /usr/local/bin/install-browsers.sh
RUN install-browsers.sh

WORKDIR /work
