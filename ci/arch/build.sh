#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# Builds the Arch Linux package in the image of
# ci/containers/arch.Containerfile, run by `cargo xtask ci --distro arch`.
#
#   /in   the filled-in PKGBUILD and the source tarball, read-only
#   /out  the package, its .SRCINFO and namcap's reports, kept on the host
#
# It runs as root: it installs the build dependencies the PKGBUILD names,
# then builds as the user `builder` in a folder of the container, so that
# nothing in /out belongs to a user the host doesn't know. namcap checks the
# PKGBUILD and the package; any error it reports fails the build, warnings
# are kept in the report.
set -eu

build=/home/builder/build
rm -rf "$build"
mkdir -p "$build" /out
cp /in/PKGBUILD /in/*.tar.gz "$build/"
chown -R builder: "$build"
# The caches are volumes, mounted as root's: hand them to the builder.
for dir in /home/builder/.cargo /home/builder/.cargo/registry /home/builder/.local \
  /home/builder/.local/share /home/builder/.local/share/pnpm /home/builder/.local/share/pnpm/store; do
  mkdir -p "$dir"
  chown builder: "$dir"
done

# The dependencies, from the PKGBUILD itself.
deps=$(bash -c 'cd "$1" && . ./PKGBUILD && printf "%s\n" "${depends[@]}" "${makedepends[@]}"' bash "$build")
# shellcheck disable=SC2086
pacman -Syu --noconfirm --needed $deps

su builder -c "cd '$build' && makepkg --printsrcinfo > .SRCINFO && makepkg --noconfirm"

package=$(find "$build" -maxdepth 1 -name 'glyphkeel-*.pkg.tar.zst' ! -name '*-debug-*' | head -1)
[ -n "$package" ] || { echo "makepkg made no package" >&2; exit 1; }

status=0
for target in "$build/PKGBUILD" "$package"; do
  name=$(basename "$target")
  namcap -i "$target" | tee "/out/namcap-$name.txt"
  # namcap's tags: E for errors, W for warnings, I for information.
  if grep -q ' E: ' "/out/namcap-$name.txt"; then
    echo "namcap reports errors for $name" >&2
    status=1
  fi
done

cp "$package" "$build/.SRCINFO" "$build/PKGBUILD" /out/
exit "$status"
