#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# Installs the Chromium that the UI's Playwright flows run in, at the
# Playwright version ui/package.json pins, into PLAYWRIGHT_BROWSERS_PATH.
# On Debian and Ubuntu Playwright installs the system libraries itself; the
# Fedora image installs them with dnf before this runs.
set -eu

: "${PLAYWRIGHT_VERSION:?}" "${PLAYWRIGHT_BROWSERS_PATH:?}"

if command -v apt-get >/dev/null 2>&1; then
  deps=--with-deps
else
  deps=
fi

cd /tmp
# shellcheck disable=SC2086
npx --yes "playwright@${PLAYWRIGHT_VERSION}" install $deps chromium
rm -rf /var/lib/apt/lists/* /root/.npm
chmod -R a+rX "$PLAYWRIGHT_BROWSERS_PATH"
ls "$PLAYWRIGHT_BROWSERS_PATH"
