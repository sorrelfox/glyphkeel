# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# CI image for the Windows build, cross-compiled from Linux. Built and run by
# `cargo xtask ci --distro windows-cross`.
#
# cargo-xwin builds for x86_64-pc-windows-msvc with clang-cl and lld-link
# against Microsoft's CRT and Windows SDK, which it downloads once (below)
# into XWIN_CACHE_DIR. Wine runs the tests, the command line and the
# installer without a display. NSIS makes the installer.
FROM registry.fedoraproject.org/fedora:45

ARG RUST_TOOLCHAIN
ARG NODE_VERSION
ARG PNPM_VERSION
ARG CARGO_DENY_VERSION

ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    COREPACK_HOME=/usr/local/corepack \
    COREPACK_ENABLE_DOWNLOAD_PROMPT=0 \
    PNPM_HOME=/usr/local/pnpm \
    CI=1
ENV PATH=/usr/local/cargo/bin:$PATH

# Build tools, the LLVM linker and librarian cargo-xwin drives, makensis, and
# Wine without its graphical extras. No display server: Wine runs headless.
RUN dnf install -y --setopt=install_weak_deps=False \
      gcc gcc-c++ make curl file git pkgconf-pkg-config tzdata xz tar \
      clang lld llvm \
      mingw32-nsis \
      wine-core wine-common \
 && dnf clean all

COPY install-toolchains.sh /usr/local/bin/install-toolchains.sh
RUN install-toolchains.sh

# The Windows target and cargo-xwin, at a pinned version.
ARG CARGO_XWIN_VERSION=0.24.2
RUN rustup target add x86_64-pc-windows-msvc \
 && cargo install --locked --version "${CARGO_XWIN_VERSION}" cargo-xwin \
 && rm -rf "${CARGO_HOME}/registry" "${CARGO_HOME}/git"

# The MSVC CRT and the Windows SDK, fetched once into the image by building a
# tiny program, so that runs need neither the download nor its time. Using
# them means accepting Microsoft's licence for them.
ENV XWIN_CACHE_DIR=/opt/xwin-cache \
    XWIN_ACCEPT_LICENSE=1
RUN mkdir -p /tmp/warm/src \
 && printf '[package]\nname = "warm"\nversion = "0.0.0"\nedition = "2024"\n' > /tmp/warm/Cargo.toml \
 && printf 'fn main() {}\n' > /tmp/warm/src/main.rs \
 && cd /tmp/warm \
 && cargo xwin build --target x86_64-pc-windows-msvc \
 && cd / && rm -rf /tmp/warm "${CARGO_HOME}/registry" "${CARGO_HOME}/git"

# Wine: no debug output and no display, so nothing ever opens a window.
ENV WINEDEBUG=-all \
    WINEDLLOVERRIDES="mscoree,mshtml=" \
    DISPLAY= \
    WAYLAND_DISPLAY=

WORKDIR /work
