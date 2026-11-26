# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# A clean Arch Linux with the package installed, as a user installs it, for
# the checks of `cargo xtask ci --distro arch`. The build context holds only
# glyphkeel.pkg.tar.zst. The checks run with no network, so every tool they
# use is installed here: the desktop and AppStream validators.
FROM docker.io/archlinux/archlinux:latest

COPY glyphkeel.pkg.tar.zst /tmp/glyphkeel.pkg.tar.zst
# The image leaves out man pages and documentation (NoExtract in pacman.conf);
# a user's system has them, and the checks look for the man page.
RUN sed -i '/^NoExtract/d' /etc/pacman.conf \
 && for try in 1 2 3; do pacman -Syu --noconfirm && break; sleep 20; done \
 && pacman -U --noconfirm /tmp/glyphkeel.pkg.tar.zst \
 && pacman -S --noconfirm --needed desktop-file-utils appstream python \
 && pacman -Scc --noconfirm && rm /tmp/glyphkeel.pkg.tar.zst
