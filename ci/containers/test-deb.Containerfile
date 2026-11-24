# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# A clean Debian or Ubuntu with the deb installed, for the checks of the
# installed package (`cargo xtask ci --suite packages,offline,perf`). BASE is
# the distribution's own image. The build context holds only glyphkeel.deb.
# The checks run with no network, so every tool they use is installed here.
ARG BASE
FROM ${BASE}

ENV DEBIAN_FRONTEND=noninteractive

COPY glyphkeel.deb /tmp/glyphkeel.deb
# The package with what it recommends, as a user gets it; then the test tools:
# a headless X server, a session bus, strace, xdotool, WebKitGTK's WebDriver,
# Python for the drivers, and the desktop and AppStream validators.
RUN apt-get -o Acquire::Retries=3 -o Acquire::http::Timeout=30 update \
 && apt-get install -y /tmp/glyphkeel.deb \
 && apt-get install -y --no-install-recommends \
      xvfb xauth dbus dbus-x11 strace xdotool procps python3 \
      webkit2gtk-driver desktop-file-utils appstream bash fonts-dejavu-core \
 && rm -rf /var/lib/apt/lists/* /tmp/glyphkeel.deb
