#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# Installs the pinned Rust toolchain, Node and pnpm into a CI image. The
# versions come from the repository (rust-toolchain.toml, .node-version and
# package.json) as build arguments, so every image matches the host's pins.
set -eu

: "${RUST_TOOLCHAIN:?}" "${NODE_VERSION:?}" "${PNPM_VERSION:?}"

case "$(uname -m)" in
  x86_64) node_arch=x64 ;;
  aarch64) node_arch=arm64 ;;
  *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

# Rust, through rustup, with only the pinned toolchain.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup-init.sh
sh /tmp/rustup-init.sh -y --no-modify-path --profile minimal \
  --default-toolchain "$RUST_TOOLCHAIN" --component rustfmt,clippy
rm /tmp/rustup-init.sh
rustc --version

# Node, the official build, checked against the release's published sums.
node_file="node-v${NODE_VERSION}-linux-${node_arch}.tar.xz"
cd /tmp
curl -sSfO "https://nodejs.org/dist/v${NODE_VERSION}/${node_file}"
curl -sSfO "https://nodejs.org/dist/v${NODE_VERSION}/SHASUMS256.txt"
grep " ${node_file}\$" SHASUMS256.txt | sha256sum -c -
tar -xJf "$node_file" -C /usr/local --strip-components=1 --no-same-owner
rm "$node_file" SHASUMS256.txt
node --version

# pnpm, through Corepack, at the version package.json names.
corepack enable pnpm
corepack install --global "pnpm@${PNPM_VERSION}"
pnpm --version
