# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# CI image: Ubuntu 24.04 LTS. Built and run by `cargo xtask ci --distro ubuntu-24.04`.
FROM docker.io/library/ubuntu:24.04

ARG RUST_TOOLCHAIN
ARG NODE_VERSION
ARG PNPM_VERSION
ARG CARGO_DENY_VERSION

ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    COREPACK_HOME=/usr/local/corepack \
    COREPACK_ENABLE_DOWNLOAD_PROMPT=0 \
    PNPM_HOME=/usr/local/pnpm \
    CI=1
ENV PATH=/usr/local/cargo/bin:$PATH

# Build tools and the desktop app's system libraries (WebKitGTK 4.1, GTK 3,
# libsoup 3), plus Xvfb for headless UI runs. tzdata explicitly: the time
# zone tests need the system's zone database, and not every base image has it.
RUN apt-get -o Acquire::Retries=3 -o Acquire::http::Timeout=30 update \
 && apt-get install -y --no-install-recommends \
      build-essential ca-certificates curl file git pkg-config tzdata xz-utils \
      libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev librsvg2-dev \
      libssl-dev \
      xvfb xauth \
 && rm -rf /var/lib/apt/lists/*

COPY install-toolchains.sh /usr/local/bin/install-toolchains.sh
RUN install-toolchains.sh

# Chromium for the UI's Playwright flows, in its own layer so that a new
# Playwright version doesn't rebuild the toolchains.
ARG PLAYWRIGHT_VERSION
ENV PLAYWRIGHT_BROWSERS_PATH=/usr/local/ms-playwright
COPY install-browsers.sh /usr/local/bin/install-browsers.sh
RUN install-browsers.sh

WORKDIR /work
