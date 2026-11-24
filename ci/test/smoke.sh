#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The installed package: both programs, every file it should bring, and a
# short session with the command line. EXPECTED_VERSION is the workspace
# version.

. /ci/lib.sh
: "${EXPECTED_VERSION:?}"

echo "== programs"
version=$(glyphkeel --version)
echo "$version"
[ "$version" = "glyphkeel $EXPECTED_VERSION" ] || fail "glyphkeel --version says '$version', expected $EXPECTED_VERSION"
for program in /usr/bin/glyphkeel /usr/bin/glyphkeel-app; do
  [ -x "$program" ] || fail "$program is missing"
  if ldd "$program" | grep 'not found'; then
    fail "$program needs libraries the package does not bring in"
  fi
done
# The command line never links the web view or GTK.
if ldd /usr/bin/glyphkeel | grep -E 'webkit|gtk'; then
  fail "glyphkeel links graphical libraries"
fi

echo "== files"
zsh_completion=
for dir in /usr/share/zsh/vendor-completions /usr/share/zsh/site-functions; do
  [ -s "$dir/_glyphkeel" ] && zsh_completion="$dir/_glyphkeel"
done
[ -n "$zsh_completion" ] || fail "the zsh completion is missing"
for file in \
  /usr/share/bash-completion/completions/glyphkeel \
  /usr/share/fish/vendor_completions.d/glyphkeel.fish \
  /usr/share/man/man1/glyphkeel.1.gz \
  /usr/share/metainfo/io.github.sorrelfox.Glyphkeel.metainfo.xml \
  /usr/share/applications/Glyphkeel.desktop; do
  [ -s "$file" ] || fail "$file is missing or empty"
  echo "ok $file"
done
grep -q '^[[:space:]]*complete .*glyphkeel$' /usr/share/bash-completion/completions/glyphkeel \
  || fail "the bash completion registers nothing"
bash -n /usr/share/bash-completion/completions/glyphkeel || fail "the bash completion does not parse"
gzip -dc /usr/share/man/man1/glyphkeel.1.gz | grep -q '^\.TH' || fail "the man page has no title"

desktop=/usr/share/applications/Glyphkeel.desktop
grep -qx 'Categories=Office;ProjectManagement;' "$desktop" || fail "the desktop file has no categories"
grep -qx 'Exec=glyphkeel-app' "$desktop" || grep -q '^Exec=glyphkeel-app ' "$desktop" \
  || fail "the desktop file does not start glyphkeel-app"
icon=$(sed -n 's/^Icon=//p' "$desktop")
ls /usr/share/icons/hicolor/*/apps/"$icon".png >/dev/null 2>&1 || fail "no icon named $icon"
if command -v desktop-file-validate >/dev/null; then
  desktop-file-validate "$desktop" || fail "desktop-file-validate"
  echo "ok desktop-file-validate"
fi
# AppStream before 1.0 (Ubuntu 22.04) doesn't know tags the file uses, such as
# <developer>; the file is validated where the tool is current.
appstream_version=$(appstreamcli --version 2>/dev/null | sed -n 's/.*version: *\([0-9]*\)\..*/\1/p')
if [ "${appstream_version:-0}" -ge 1 ]; then
  # --no-net: the screenshot and the links are not fetched.
  appstreamcli validate --no-net --explain /usr/share/metainfo/io.github.sorrelfox.Glyphkeel.metainfo.xml \
    || fail "appstreamcli validate"
  echo "ok appstreamcli validate"
fi

echo "== command line"
glyphkeel project create --name Smoke --key SM
glyphkeel task create --project SM --title "Check the installed package"
listing=$(glyphkeel task list --project SM)
echo "$listing"
echo "$listing" | grep -q 'SM-1 .*Check the installed package' || fail "the new task is not listed"
glyphkeel --json task show SM-1 | grep -q '"key": *"SM-1"' || fail "task show SM-1"
echo "smoke: passed"
