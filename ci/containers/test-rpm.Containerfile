# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# A clean Fedora with the rpm installed, for the checks of the installed
# package (`cargo xtask ci --suite packages,offline,perf`). BASE is the
# distribution's own image. The build context holds only glyphkeel.rpm.
# The checks run with no network, so every tool they use is installed here.
ARG BASE
FROM ${BASE}

COPY glyphkeel.rpm /tmp/glyphkeel.rpm
# The package with what it recommends, as a user gets it; then the test tools:
# a headless X server, a session bus, strace, xdotool, WebKitGTK's WebDriver
# (packaged with the GTK 4 build, the same WebKit version), Python for the
# drivers, and the desktop and AppStream validators.
RUN dnf install -y /tmp/glyphkeel.rpm \
 && dnf install -y --setopt=install_weak_deps=False \
      xorg-x11-server-Xvfb xorg-x11-xauth dbus-daemon dbus-tools strace xdotool \
      procps-ng python3 webkitgtk6.0 desktop-file-utils appstream bash \
      dejavu-sans-fonts mesa-dri-drivers \
 && dnf clean all && rm /tmp/glyphkeel.rpm
