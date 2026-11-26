#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The signed DNF repository, as a user adds it (`cargo xtask ci --suite
# repos`). It runs in a clean Fedora container with the network, for the
# package's dependencies, and the repository at /repo, made by
# `cargo xtask repos --site-url file:///repo` with a throwaway key.

. /ci/lib.sh

echo "== the repository file, as the documentation says"
cp /repo/rpm/glyphkeel.repo /etc/yum.repos.d/glyphkeel.repo
cat /etc/yum.repos.d/glyphkeel.repo

echo "== the package in the repository is signed"
rpm --import /repo/rpm/glyphkeel.asc
for package in /repo/rpm/*.rpm; do
  rpm --checksig "$package" | tee -a /out/rpm-checksig.log
  rpm --checksig "$package" | grep -q 'digests signatures OK' || fail "$package is not signed with the key"
done
# Removed again: dnf must import it itself, from the .repo file's gpgkey.
fingerprint=$(gpg --show-keys --with-colons /repo/rpm/glyphkeel.asc 2>/dev/null | awk -F: '$1 == "fpr" { print $10; exit }')
if [ -n "$fingerprint" ] && command -v rpmkeys >/dev/null && rpmkeys --help 2>&1 | grep -q -- '--delete'; then
  rpmkeys --delete "$fingerprint"
else
  rpm --erase "$(rpm -q gpg-pubkey --qf '%{NAME}-%{VERSION}-%{RELEASE}\n' | tail -1)"
fi

echo "== install; dnf checks the metadata's signature and imports the key"
dnf install -y glyphkeel >/out/dnf-install.log 2>&1 || { tail -30 /out/dnf-install.log; fail "dnf install glyphkeel"; }
offered=$(rpm -q --qf '%{VERSION}' glyphkeel)
version=$(glyphkeel --version)
[ "$version" = "glyphkeel $offered" ] || fail "glyphkeel --version says '$version', the package is $offered"
[ -x /usr/bin/glyphkeel-app ] || fail "the app is missing"
echo "ok $version"

echo "== changed metadata is refused"
rm -rf /tmp/bad && cp -r /repo/rpm /tmp/bad
printf '\n' >> /tmp/bad/repodata/repomd.xml
sed -e 's|^\[glyphkeel\]|[glyphkeel-bad]|' -e 's|^baseurl=.*|baseurl=file:///tmp/bad/|' \
  /repo/rpm/glyphkeel.repo > /etc/yum.repos.d/glyphkeel-bad.repo
# A repository that fails is an error, not skipped; the key is accepted, so
# only the signature can stop it.
echo skip_if_unavailable=False >> /etc/yum.repos.d/glyphkeel-bad.repo
if dnf -y --repo=glyphkeel-bad makecache >/out/dnf-tampered.log 2>&1; then
  cat /out/dnf-tampered.log
  fail "dnf accepted changed metadata"
fi
grep -q 'signature verification error' /out/dnf-tampered.log || { cat /out/dnf-tampered.log; fail "dnf refused the changed metadata, but not for its signature"; }
tail -2 /out/dnf-tampered.log
echo "repo-dnf: passed"
