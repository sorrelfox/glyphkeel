# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The tools of `cargo xtask repos`, which builds the signed APT and DNF
# repositories: apt-ftparchive, dpkg-deb, createrepo_c, rpm and rpmsign, and
# gpg, on the same Ubuntu as the release workflow's runners, plus the pinned
# Rust toolchain for the task runner. Built and run by
# `cargo xtask ci --suite repos`.
FROM docker.io/library/ubuntu:24.04

ARG RUST_TOOLCHAIN

ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    DEBIAN_FRONTEND=noninteractive
ENV PATH=/usr/local/cargo/bin:$PATH

RUN apt-get update -q \
 && apt-get install -y -q --no-install-recommends \
      apt-utils dpkg-dev createrepo-c rpm gnupg gzip \
      ca-certificates curl gcc libc6-dev \
 && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup-init.sh \
 && sh /tmp/rustup-init.sh -y --no-modify-path --profile minimal \
      --default-toolchain "$RUST_TOOLCHAIN" \
 && rm /tmp/rustup-init.sh \
 && rustc --version

WORKDIR /work
