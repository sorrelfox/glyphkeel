# Making a release

Releases are built by GitHub Actions, and only when a release tag is pushed.
Pushes to branches and pull requests start no runs.

1. On `main`, set the final version everywhere (`Cargo.toml`,
   `ui/package.json`, `crates/gk-app/tauri.conf.json`) and write its section
   in `CHANGELOG.md`.
2. Push a pre-release tag such as `v0.1.0-rc.1`. The pre-release workflow
   (`.github/workflows/prerelease.yml`) runs every suite, builds the deb, the
   rpm, the Arch Linux package, the Flatpak bundle and the Windows installer
   (NSIS, per-user, unsigned), and publishes them as a GitHub pre-release
   with the source tarball, the files for the AUR and Flathub, an SBOM,
   `SHA256SUMS` and build-provenance attestations.
3. When the pre-release is good, push the final tag (`v0.1.0`) on the same
   commit. The release workflow (`.github/workflows/release.yml`) builds
   nothing: it checks the pre-release's files and republishes them unchanged.
   Then it publishes the Flatpak, APT and DNF repositories on GitHub Pages
   (below).

Timings and memory are measured in the pre-release, but only reported: the
table is in the run's summary and in the `check-results` artifact. The
budgets are checked on the reference machine with
`cargo xtask ci --suite perf --perf-enforce`.

## The Flatpak

The pre-release workflow builds the Flatpak from source with
`crates/gk-app/packaging/flatpak/io.github.sorrelfox.Glyphkeel.yml`. The
manifest's offline source lists (`cargo-sources.json`, `node-sources.json`)
are generated in the job from `Cargo.lock` and `pnpm-lock.yaml` by
[flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools),
pinned to a commit in the workflow. They are not kept in the repository. The
bundle, `glyphkeel-<version>-x86_64.flatpak`, is one of the pre-release's
files.

The release workflow imports that same bundle into an OSTree repository,
signs it with the project's key and publishes it on GitHub Pages:

| What | Address |
| --- | --- |
| Repository | `https://sorrelfox.github.io/glyphkeel/flatpak/repo/` |
| Install file | `https://sorrelfox.github.io/glyphkeel/flatpak/io.github.sorrelfox.Glyphkeel.flatpakref` |
| Remote file | `https://sorrelfox.github.io/glyphkeel/flatpak/glyphkeel.flatpakrepo` |
| Public key | `https://sorrelfox.github.io/glyphkeel/flatpak/glyphkeel.gpg` |

The `.flatpakref` and `.flatpakrepo` are kept in
`crates/gk-app/packaging/flatpak/`; the workflow adds the public key to them
when it publishes them. The repository holds the latest release only, on the
branch `stable`; the runtime comes from Flathub.

Users install it with:

```sh
flatpak install --user https://sorrelfox.github.io/glyphkeel/flatpak/io.github.sorrelfox.Glyphkeel.flatpakref
```

and get updates with `flatpak update`. The bundle on the releases page
installs without the repository (`flatpak install --user ./glyphkeel-….flatpak`)
but doesn't update itself.

A Pages deployment replaces the whole site. The site is built by the release
workflow: the project's website from `site/` when the repository has that
folder, otherwise a short start page, plus `flatpak/`. So the website changes
only with a release.

To build the Flatpak on your own machine, generate the two source lists as
the manifest's header says and run `cargo xtask ci --suite flatpak`. Without
them, that suite is skipped with a message.

### One-time set-up

Until these steps are done, a release still publishes its files, but the
release workflow leaves the Flatpak repository out and shows a warning
saying the signing key is missing. Nothing unsigned is ever published.

1. **Make a signing key**, on your own computer, only for the Flatpak
   repository. It has no passphrase, because the workflow can't type one;
   GitHub's secret store protects it.

   ```sh
   export GNUPGHOME="$(mktemp -d)"
   gpg --batch --passphrase '' --quick-generate-key \
     "Glyphkeel Flatpak repository" ed25519 sign never
   gpg --list-secret-keys          # note the fingerprint
   gpg --armor --export-secret-keys FINGERPRINT > glyphkeel-flatpak-private.asc
   gpg --armor --export FINGERPRINT > glyphkeel-flatpak-public.asc
   ```

   Keep `glyphkeel-flatpak-private.asc` somewhere safe and offline (a
   password manager, for example). Losing it means every user has to add the
   repository again with a new key; leaking it means someone else could
   sign updates. Delete the temporary `GNUPGHOME` folder afterwards.

2. **Add the secret.** On GitHub: the repository's *Settings → Secrets and
   variables → Actions → New repository secret*. Name it
   `FLATPAK_GPG_PRIVATE_KEY` and paste the whole content of
   `glyphkeel-flatpak-private.asc`, from `-----BEGIN PGP PRIVATE KEY BLOCK-----`
   to the `END` line. The public key needs no secret: the workflow takes it
   from the private key.

3. **Turn on GitHub Pages.** *Settings → Pages → Build and deployment →
   Source:* choose **GitHub Actions**. Not "Deploy from a branch": the
   workflow uploads the site itself.

4. **Let release tags deploy to Pages.** Turning on Pages creates an
   environment called `github-pages` that, by default, accepts deployments
   only from the default branch, while releases run from tags. In *Settings →
   Environments → github-pages → Deployment branches and tags*, add a rule of
   type **Tag** with the pattern `v*`. Keep the existing rule for
   `main`.

The next final release then publishes the repository. If a release was made
before these steps, push nothing again: make the next release as usual.

### Changing the key

Make a new key as above and replace the secret. The next release signs with
the new key and publishes it in the `.flatpakref` and `.flatpakrepo`, but
people who already added the repository keep the old key and their updates
fail until they add the repository again:

```sh
flatpak remote-modify --user --gpg-import=glyphkeel-flatpak-public.asc glyphkeel
```

So change the key only if it was lost or leaked.

## The APT and DNF repositories

The release workflow also publishes the release's deb and rpm as signed
repositories on GitHub Pages, next to the Flatpak repository:

| What | Address |
| --- | --- |
| APT repository (suite `stable`, component `main`) | `https://sorrelfox.github.io/glyphkeel/apt/` |
| APT source entry and key | `apt/glyphkeel.sources`, `apt/glyphkeel.gpg` |
| DNF repository | `https://sorrelfox.github.io/glyphkeel/rpm/` |
| DNF repository file and key | `rpm/glyphkeel.repo`, `rpm/glyphkeel.asc` |

`cargo xtask repos` builds both from one deb and one rpm with
apt-ftparchive, createrepo_c and gpg: the APT `Release` file is signed
(`InRelease` and `Release.gpg`), and so are the DNF metadata
(`repomd.xml.asc`) and the rpm's copy in the repository, so `dnf` checks
both (`gpgcheck` and `repo_gpgcheck`). The release's own rpm stays as it was
built. Like the Flatpak repository, they hold the latest release only.
[Getting started](guide/getting-started.md#installing-on-linux) shows how
users add them.

`cargo xtask ci --suite repos` checks the whole path in containers: it
builds the repositories with a throwaway key, adds them in clean containers
of each distribution as the documentation says, installs Glyphkeel from them,
and checks that a changed `InRelease` or `repomd.xml` is refused.
`--packages-from DIR` takes a deb and an rpm from DIR instead of building
them.

### One-time set-up

Without this the release workflow leaves the APT and DNF repositories out,
with a warning.

1. **Make a signing key** for the package repositories, as for the Flatpak
   (above), with its own name:

   ```sh
   export GNUPGHOME="$(mktemp -d)"
   gpg --batch --passphrase '' --quick-generate-key \
     "Glyphkeel package repositories" ed25519 sign never
   gpg --armor --export-secret-keys FINGERPRINT > glyphkeel-packages-private.asc
   ```

2. **Add the secret** `PACKAGES_GPG_PRIVATE_KEY` with the whole content of
   `glyphkeel-packages-private.asc`. Pages must be set up as for the Flatpak.

Changing this key later means every user must fetch the new key again, the
same way they added it.

## The Arch Linux package and the AUR

`crates/gk-app/packaging/arch/PKGBUILD` builds the package from the
release's source tarball, `glyphkeel-<version>.tar.gz`, which
`cargo xtask source-tarball` makes from the files git lists, reproducibly.
`cargo xtask ci --distro arch` fills in the version, the release number and
the tarball's checksum, builds the package with makepkg in an Arch Linux
container, checks the PKGBUILD and the package with namcap (an error fails
the build), and installs the package in a clean Arch Linux container, where
it checks the files, a command-line session and an MCP session. Everything
lands in `target/ci-cache/arch/out/`.

The pre-release attaches the package, the source tarball and
`glyphkeel-<version>-aur.tar.gz`, which holds the filled-in `PKGBUILD` and
its `.SRCINFO`. To update the AUR package after the final release:

```sh
git clone ssh://aur@aur.archlinux.org/glyphkeel.git
tar -xzf glyphkeel-<version>-aur.tar.gz   # writes glyphkeel/PKGBUILD and glyphkeel/.SRCINFO
git -C glyphkeel add PKGBUILD .SRCINFO
git -C glyphkeel commit -m "Update to <version>"
git -C glyphkeel push
```

The PKGBUILD's source is the tarball on the final release's page, so push
only after the final release is published.

## Flathub

The pre-release also attaches `glyphkeel-<version>-flathub.tar.gz`, the files
of the Flathub repository for that release, written by `cargo xtask flathub`:
the manifest with a `git` source naming the tag `v<version>` and its commit
(instead of this repository's folder) and without `default-branch`,
`flathub.json` (x86_64 only, as the manifest's Rust toolchain is), and the
generated `cargo-sources.json` and `node-sources.json`.

`cargo xtask ci --suite flathub` checks them in a container: it generates the
source lists with the pinned flatpak-builder-tools, lints the manifest and
the AppStream file with Flathub's linter (`flatpak-builder-lint` from
`org.flatpak.Builder`), builds the manifest with flatpak-builder from this
checkout, runs `appstreamcli validate` and `desktop-file-validate` inside the
build, and lints the built repository. Its reports are in
`target/ci-cache/flathub/`.

The first submission is a pull request to
[flathub/flathub](https://github.com/flathub/flathub) on the branch
`new-pr`, with these files at the top level; Flathub's reviewers then create
the app's own repository. Later releases are pull requests to that
repository with the new archive's files.
