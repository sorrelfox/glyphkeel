#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The signed APT repository, as a user adds it (`cargo xtask ci --suite
# repos`). It runs in a clean Debian or Ubuntu container with the network,
# for the package's dependencies, and the repository at /repo, made by
# `cargo xtask repos --site-url file:///repo` with a throwaway key.

. /ci/lib.sh

echo "== the repository's key and source entry, as the documentation says"
mkdir -p /etc/apt/keyrings
cp /repo/apt/glyphkeel.gpg /etc/apt/keyrings/glyphkeel.gpg
cp /repo/apt/glyphkeel.sources /etc/apt/sources.list.d/glyphkeel.sources
cat /etc/apt/sources.list.d/glyphkeel.sources

echo "== apt-get update checks the signature"
export DEBIAN_FRONTEND=noninteractive
apt-get update -q 2>&1 | tee /out/apt-update.log
if grep -E -q 'NO_PUBKEY|not signed|BADSIG|is not valid|^[EW]: .*glyphkeel' /out/apt-update.log; then
  fail "apt-get update did not accept the repository"
fi
offered=$(apt-cache policy glyphkeel | sed -n 's/^  Candidate: //p')
if [ -z "$offered" ] || [ "$offered" = "(none)" ]; then
  fail "the repository offers no glyphkeel"
fi
echo "offered: $offered"

echo "== install"
apt-get install -y -q glyphkeel >/out/apt-install.log 2>&1 || { tail -30 /out/apt-install.log; fail "apt-get install glyphkeel"; }
version=$(glyphkeel --version)
[ "$version" = "glyphkeel $offered" ] || fail "glyphkeel --version says '$version', the repository offers $offered"
[ -x /usr/bin/glyphkeel-app ] || fail "the app is missing"
echo "ok $version"

echo "== a changed Release file is refused"
rm -rf /tmp/bad && cp -r /repo/apt /tmp/bad
sed -i 's/^Origin: Glyphkeel$/Origin: Someone else/' /tmp/bad/dists/stable/InRelease
rm /tmp/bad/dists/stable/Release /tmp/bad/dists/stable/Release.gpg
sed 's|^URIs: .*|URIs: file:///tmp/bad/|' /repo/apt/glyphkeel.sources > /etc/apt/sources.list.d/glyphkeel.sources
if apt-get update -q >/out/apt-tampered.log 2>&1 && ! grep -E -q 'BADSIG|not signed|is not valid|^[EW]: ' /out/apt-tampered.log; then
  cat /out/apt-tampered.log
  fail "apt-get update accepted a changed Release file"
fi
grep -E '^[EW]: ' /out/apt-tampered.log | head -3
echo "repo-apt: passed"
