#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The Flathub check in the image of ci/containers/flathub.Containerfile, run
# by `cargo xtask ci --suite flathub` with the source copied to /work.
#
#   /out/stage/flathub.yml  the manifest Flathub gets (git source), linted
#   /out/stage/local.yml    the same with this checkout as its source, built
#   /out/submission/        the Flathub repository's files, when all passed
#   /out/*.txt              the reports
#   /cache                  flatpak-builder's state and the build, kept
#
# BUILDER_TOOLS_REPO and BUILDER_TOOLS_COMMIT pin flatpak-builder-tools, as
# the pre-release workflow does.
set -eu
: "${BUILDER_TOOLS_REPO:?}" "${BUILDER_TOOLS_COMMIT:?}" "${APP_ID:?}" "${FLATPAK_DIR:?}"

manifest_dir="/work/$FLATPAK_DIR"
step() { echo "==> [flathub] $*"; }

step "flatpak-builder-tools at $BUILDER_TOOLS_COMMIT"
tools=/tmp/flatpak-builder-tools
git init --quiet "$tools"
git -C "$tools" fetch --quiet --depth 1 "$BUILDER_TOOLS_REPO" "$BUILDER_TOOLS_COMMIT"
git -C "$tools" checkout --quiet --detach FETCH_HEAD
test "$(git -C "$tools" rev-parse HEAD)" = "$BUILDER_TOOLS_COMMIT"
/opt/generators/bin/pip install --quiet --no-deps "$tools/node"

step "the offline source lists from Cargo.lock and pnpm-lock.yaml"
/opt/generators/bin/python "$tools/cargo/flatpak-cargo-generator.py" \
  /work/Cargo.lock -o "$manifest_dir/cargo-sources.json"
/opt/generators/bin/flatpak-node-generator pnpm /work/pnpm-lock.yaml \
  --pnpm-store-version v11 -o "$manifest_dir/node-sources.json"
cp /out/stage/local.yml "$manifest_dir/flathub-local.yml"

# `flatpak run` needs the system bus and a session bus; the container has
# neither of its own.
mkdir -p /run/dbus
dbus-daemon --system --fork

step "Flathub and its linter (org.flatpak.Builder)"
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user --noninteractive --or-update flathub org.flatpak.Builder

lint() {
  # flatpak-builder-lint's exit code says whether it found errors; its JSON
  # report goes to the named file.
  report=$1
  shift
  if dbus-run-session -- flatpak run --filesystem=/out --filesystem=/work --filesystem=/cache \
      --command=flatpak-builder-lint org.flatpak.Builder "$@" >"/out/$report" 2>&1; then
    echo "ok $report"
  else
    cat "/out/$report"
    return 1
  fi
}

status=0
submission=/out/submission
mkdir -p "$submission"
cp /out/stage/flathub.yml "$submission/$APP_ID.yml"
cp "$manifest_dir/flathub.json" "$manifest_dir/cargo-sources.json" \
  "$manifest_dir/node-sources.json" "$submission/"

step "lint: the manifest Flathub gets"
lint lint-manifest.txt manifest "$submission/$APP_ID.yml" || status=1
step "lint: the AppStream file"
# The linter fetches every link of the file. Links that don't resolve yet
# (the project site and the screenshots on the branch main exist only once a
# release is published) are reported, not failed; anything else fails.
if ! lint lint-appstream.txt appstream "$manifest_dir/../$APP_ID.metainfo.xml"; then
  others=$(grep -E '^[EW]: ' /out/lint-appstream.txt \
    | grep -v -E ': (url-not-reachable|screenshot-image-not-found)$' || true)
  if [ -n "$others" ]; then
    status=1
  else
    echo "note: only unreachable links, which exist once a release is published"
  fi
fi

step "flatpak-builder: the manifest, from this checkout"
flatpak-builder --user --disable-rofiles-fuse --force-clean --ccache \
  --install-deps-from=flathub --state-dir=/cache/state --repo=/cache/repo \
  /cache/build "$manifest_dir/flathub-local.yml" >/out/build.txt 2>&1 \
  || { tail -60 /out/build.txt; echo "the build FAILED (log: build.txt)"; exit 1; }
tail -5 /out/build.txt

in_build() {
  flatpak-builder --run /cache/build "$manifest_dir/flathub-local.yml" "$@"
}
step "in the build: AppStream, the desktop file, the command line"
in_build appstreamcli validate --no-net --explain "/app/share/metainfo/$APP_ID.metainfo.xml" \
  | tee /out/appstreamcli.txt || status=1
in_build desktop-file-validate "/app/share/applications/$APP_ID.desktop" || status=1
version=$(in_build glyphkeel --version || true)
echo "$version"
case "$version" in "glyphkeel "*) ;; *) echo "glyphkeel --version: '$version'"; status=1 ;; esac
for file in LICENSE THIRD-PARTY-NOTICES.txt; do
  in_build test -s "/app/share/licenses/$APP_ID/$file" || { echo "$file is missing"; status=1; }
done

step "lint: the built repository"
# Flathub's build fetches the screenshots into the repository; here they
# can't be fetched until a release is published, so the errors that follow
# from that alone are reported, not failed.
if ! lint lint-repo.txt repo /cache/repo; then
  if python3 - /out/lint-repo.txt <<'PY'
import json, sys
text = open(sys.argv[1]).read()
report = json.loads(text[text.index("{"):])
allowed = {"appstream-screenshots-not-mirrored-in-ostree", "appstream-missing-screenshots"}
sys.exit(0 if set(report.get("errors", [])) <= allowed and not report.get("warnings") else 1)
PY
  then
    echo "note: only the screenshots, which Flathub's build mirrors once they are published"
  else
    status=1
  fi
fi

if [ "$status" -ne 0 ]; then
  echo "flathub: FAILED (reports in /out)"
  exit 1
fi
echo "flathub: passed"
