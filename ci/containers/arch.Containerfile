# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The Arch Linux build image: makepkg, namcap and an unprivileged user to
# build as (makepkg refuses to run as root). The package's own build
# dependencies are installed when it runs, from the PKGBUILD. Built and run by
# `cargo xtask ci --distro arch`.
FROM docker.io/archlinux/archlinux:base-devel

# The mirrors time out now and then: a few tries before giving up.
RUN for try in 1 2 3; do pacman -Syu --noconfirm --needed namcap && break; sleep 20; done \
 && pacman -Q namcap \
 && pacman -Scc --noconfirm \
 && useradd --create-home builder
