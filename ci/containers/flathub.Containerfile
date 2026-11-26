# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The Flathub build check (`cargo xtask ci --suite flathub`): flatpak and
# flatpak-builder on the Ubuntu of the pre-release workflow's runners, plus
# what the offline source generators of flatpak-builder-tools need (Python
# with aiohttp, PyYAML and tomlkit at the versions the workflow pins, and
# git). The runtime, the SDK and Flathub's linter (org.flatpak.Builder) are
# installed when it runs, into a volume.
FROM docker.io/library/ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update -q \
 && apt-get install -y -q --no-install-recommends \
      flatpak flatpak-builder git ca-certificates python3 python3-venv ccache xz-utils \
      dbus-daemon binutils elfutils patch unzip zstd \
 && rm -rf /var/lib/apt/lists/* \
 && python3 -m venv /opt/generators \
 && /opt/generators/bin/pip install --quiet aiohttp==3.13.3 PyYAML==6.0.3 tomlkit==0.13.3

WORKDIR /work
