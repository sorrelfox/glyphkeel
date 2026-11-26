// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask notices [--check]`: THIRD-PARTY-NOTICES.txt, the notices of
//! everything the programs and the app's interface contain.
//!
//! - The crates both programs link on the shipped targets, from
//!   `cargo metadata` (normal and build dependencies, never development
//!   ones), with the licence files each crate brings.
//! - The npm packages of the interface's production dependencies, from
//!   `pnpm licenses list --prod`, with theirs; and the parts of development
//!   packages that the bundler puts into the built interface.
//! - The other components listed in licences.toml.
//!
//! Every licence is checked against the policy in licences.toml. With
//! `--check` nothing is written, and the command fails when the file is out
//! of date; it fails in both modes when a licence isn't allowed.

mod spdx;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use self::spdx::Expr;

/// The generated file, relative to the repository.
pub(crate) const OUTPUT: &str = "THIRD-PARTY-NOTICES.txt";
/// The policy and the extra entries.
const POLICY: &str = "licences.toml";
/// cargo-deny's configuration, whose licence list must match the policy's.
const DENY: &str = "deny.toml";
/// Standard licence texts, for packages that bring no licence file.
const STANDARD_TEXTS: &str = "xtask/licences";

pub(crate) fn run(root: &Path, args: &[String]) -> Result<bool> {
    let check = match args {
        [] => false,
        [flag] if flag == "--check" => true,
        other => bail!("`notices` takes only --check (got: {})", other.join(" ")),
    };
    let policy = Policy::read(root)?;
    let mut problems = policy.matches_deny(root)?;
    let mut entries = crates(root)?;
    entries.extend(npm_packages(root, &policy)?);
    entries.extend(policy.bundled_packages(root)?);
    entries.extend(policy.components(root)?);
    problems.extend(policy.problems(&entries));
    let text = render(&entries)?;

    let path = root.join(OUTPUT);
    let current = fs::read_to_string(&path).ok();
    let fresh = current.as_deref() == Some(text.as_str());
    if !fresh {
        if check {
            problems.push(format!(
                "{OUTPUT} is out of date: run `cargo xtask notices`"
            ));
        } else {
            fs::write(&path, &text).with_context(|| format!("cannot write {}", path.display()))?;
            eprintln!("notices: wrote {OUTPUT}");
        }
    }
    for problem in &problems {
        eprintln!("notices: {problem}");
    }
    if problems.is_empty() {
        let count = |kind| entries.iter().filter(|e| e.kind == kind).count();
        eprintln!(
            "notices: {} crates, {} npm packages, {} components, every licence allowed",
            count(Kind::Crate),
            count(Kind::Npm),
            count(Kind::Component)
        );
    }
    Ok(problems.is_empty())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Crate,
    Npm,
    Component,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Crate => "crate",
            Self::Npm => "npm",
            Self::Component => "component",
        }
    }

    fn parse(name: &str) -> Result<Self> {
        match name {
            "crate" => Ok(Self::Crate),
            "npm" => Ok(Self::Npm),
            "component" => Ok(Self::Component),
            other => bail!("unknown kind `{other}` (crate, npm or component)"),
        }
    }
}

/// One package or component in the notices.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    kind: Kind,
    name: String,
    version: Option<String>,
    /// The SPDX expression, or `None` when the package names no licence.
    licence: Option<String>,
    /// The project's address.
    source: Option<String>,
    /// The package's page in its registry.
    package: Option<String>,
    /// What it is used for (components only).
    used: Option<String>,
    copyright: Option<String>,
    /// Text shown with the entry (components only).
    note: Option<String>,
    /// The licence files: a label and the text.
    texts: Vec<Text>,
    /// Whether the texts are standard ones, because the package has no
    /// licence file of its own.
    standard: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Text {
    label: String,
    body: String,
}

impl Entry {
    fn title(&self) -> String {
        match &self.version {
            Some(version) => format!("{} {version}", self.name),
            None => self.name.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// The policy

#[derive(Debug)]
struct Policy {
    allow: BTreeSet<String>,
    exceptions: Vec<Exception>,
    overrides: Vec<Override>,
    bundled: Vec<Bundled>,
    components: Vec<Component>,
}

#[derive(Debug)]
struct Exception {
    kind: Kind,
    name: String,
    allow: Vec<String>,
}

#[derive(Debug)]
struct Override {
    kind: Kind,
    name: String,
    licence: String,
}

#[derive(Debug)]
struct Bundled {
    name: String,
    path: String,
}

#[derive(Debug)]
struct Component {
    name: String,
    licence: String,
    source: Option<String>,
    used: Option<String>,
    copyright: Option<String>,
    note: Option<String>,
    texts: Vec<String>,
}

fn table_str(table: &toml::Table, key: &str) -> Option<String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
}

fn table_strings(table: &toml::Table, key: &str) -> Vec<String> {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn tables<'a>(table: &'a toml::Table, key: &str) -> impl Iterator<Item = &'a toml::Table> {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_table)
}

fn need(table: &toml::Table, key: &str, what: &str) -> Result<String> {
    table_str(table, key).with_context(|| format!("{POLICY}: a `{what}` has no `{key}`"))
}

impl Policy {
    fn read(root: &Path) -> Result<Self> {
        let text = fs::read_to_string(root.join(POLICY))
            .with_context(|| format!("cannot read {POLICY}"))?;
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self> {
        let table: toml::Table = text
            .parse()
            .with_context(|| format!("cannot parse {POLICY}"))?;
        let allow: BTreeSet<String> = table_strings(&table, "allow").into_iter().collect();
        if allow.is_empty() {
            bail!("{POLICY} allows no licence");
        }
        let mut exceptions = Vec::new();
        for t in tables(&table, "exception") {
            need(t, "reason", "exception")?;
            exceptions.push(Exception {
                kind: Kind::parse(&need(t, "kind", "exception")?)?,
                name: need(t, "name", "exception")?,
                allow: table_strings(t, "allow"),
            });
        }
        let mut overrides = Vec::new();
        for t in tables(&table, "override") {
            need(t, "reason", "override")?;
            overrides.push(Override {
                kind: Kind::parse(&need(t, "kind", "override")?)?,
                name: need(t, "name", "override")?,
                licence: need(t, "licence", "override")?,
            });
        }
        let mut bundled = Vec::new();
        for t in tables(&table, "bundled") {
            need(t, "reason", "bundled")?;
            bundled.push(Bundled {
                name: need(t, "name", "bundled")?,
                path: need(t, "path", "bundled")?,
            });
        }
        let mut components = Vec::new();
        for t in tables(&table, "component") {
            components.push(Component {
                name: need(t, "name", "component")?,
                licence: need(t, "licence", "component")?,
                source: table_str(t, "source"),
                used: table_str(t, "used"),
                copyright: table_str(t, "copyright"),
                note: table_str(t, "note"),
                texts: table_strings(t, "texts"),
            });
        }
        Ok(Self {
            allow,
            exceptions,
            overrides,
            bundled,
            components,
        })
    }

    /// The licence list in deny.toml must be the same as this one.
    fn matches_deny(&self, root: &Path) -> Result<Vec<String>> {
        let text =
            fs::read_to_string(root.join(DENY)).with_context(|| format!("cannot read {DENY}"))?;
        let table: toml::Table = text
            .parse()
            .with_context(|| format!("cannot parse {DENY}"))?;
        let deny: BTreeSet<String> = table
            .get("licenses")
            .and_then(toml::Value::as_table)
            .map(|t| table_strings(t, "allow"))
            .unwrap_or_default()
            .into_iter()
            .collect();
        Ok(self.compare_allow(&deny))
    }

    fn compare_allow(&self, deny: &BTreeSet<String>) -> Vec<String> {
        let mut problems = Vec::new();
        let only_here: Vec<&String> = self.allow.difference(deny).collect();
        let only_deny: Vec<&String> = deny.difference(&self.allow).collect();
        if !only_here.is_empty() {
            problems.push(format!(
                "allowed in {POLICY} but not in {DENY}: {}",
                join(&only_here)
            ));
        }
        if !only_deny.is_empty() {
            problems.push(format!(
                "allowed in {DENY} but not in {POLICY}: {}",
                join(&only_deny)
            ));
        }
        problems
    }

    fn override_for(&self, kind: Kind, name: &str) -> Option<&str> {
        self.overrides
            .iter()
            .find(|o| o.kind == kind && o.name == name)
            .map(|o| o.licence.as_str())
    }

    /// Every entry whose licence isn't allowed, and every exception or
    /// override that no entry needs any more.
    fn problems(&self, entries: &[Entry]) -> Vec<String> {
        let mut problems = Vec::new();
        let mut used_exceptions = BTreeSet::new();
        for entry in entries {
            let Some(licence) = &entry.licence else {
                problems.push(format!(
                    "{} {} names no licence: add an override to {POLICY} after reading its licence file",
                    entry.kind.name(),
                    entry.title()
                ));
                continue;
            };
            let expr = match Expr::parse(licence) {
                Ok(expr) => expr,
                Err(err) => {
                    problems.push(format!(
                        "{} {}: cannot read the licence `{licence}`: {err:#}",
                        entry.kind.name(),
                        entry.title()
                    ));
                    continue;
                }
            };
            let extra: Vec<(usize, &Exception)> = self
                .exceptions
                .iter()
                .enumerate()
                .filter(|(_, e)| e.kind == entry.kind && e.name == entry.name)
                .collect();
            let allowed = |id: &str| {
                self.allow.contains(id)
                    || extra.iter().any(|(_, e)| e.allow.iter().any(|a| a == id))
            };
            if expr.allowed(&allowed) {
                // An exception counts as used when the licence needs it.
                let plain = |id: &str| self.allow.contains(id);
                if !expr.allowed(&plain) {
                    used_exceptions.extend(extra.iter().map(|(i, _)| *i));
                }
            } else {
                problems.push(format!(
                    "{} {} has the licence `{licence}`, which {POLICY} doesn't allow",
                    entry.kind.name(),
                    entry.title()
                ));
            }
        }
        for (i, exception) in self.exceptions.iter().enumerate() {
            if !used_exceptions.contains(&i) {
                problems.push(format!(
                    "the exception for {} {} in {POLICY} is no longer needed",
                    exception.kind.name(),
                    exception.name
                ));
            }
        }
        for o in &self.overrides {
            if !entries.iter().any(|e| e.kind == o.kind && e.name == o.name) {
                problems.push(format!(
                    "the override for {} {} in {POLICY} names nothing that ships",
                    o.kind.name(),
                    o.name
                ));
            }
        }
        problems
    }

    fn bundled_packages(&self, root: &Path) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        for bundled in &self.bundled {
            let dir = root.join(&bundled.path);
            let mut entry = npm_entry(root, &dir, self)
                .with_context(|| format!("{POLICY}: cannot read {}", bundled.path))?;
            if entry.name != bundled.name {
                bail!(
                    "{POLICY}: {} holds {}, not {}",
                    bundled.path,
                    entry.name,
                    bundled.name
                );
            }
            entry.kind = Kind::Npm;
            out.push(entry);
        }
        Ok(out)
    }

    fn components(&self, root: &Path) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        for c in &self.components {
            let mut texts = Vec::new();
            for file in &c.texts {
                let body = fs::read_to_string(root.join(file))
                    .with_context(|| format!("{POLICY}: cannot read {file}"))?;
                texts.push(Text {
                    label: format!("standard text: {}", file_stem(file)),
                    body: normalise(&body),
                });
            }
            out.push(Entry {
                kind: Kind::Component,
                name: c.name.clone(),
                version: None,
                licence: Some(c.licence.clone()),
                source: c.source.clone(),
                package: None,
                used: c.used.clone(),
                copyright: c.copyright.clone(),
                note: c.note.clone(),
                texts,
                standard: false,
            });
        }
        Ok(out)
    }
}

fn join(items: &[&String]) -> String {
    items
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn file_stem(path: &str) -> &str {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
}

// ---------------------------------------------------------------------------
// Licence files

/// Whether a file name is one of a licence or notice file.
fn is_licence_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let prefixed = [
        "license",
        "licence",
        "copying",
        "notice",
        "unlicense",
        "copyright",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    let code = [
        ".rs", ".js", ".mjs", ".cjs", ".ts", ".mts", ".cts", ".json", ".toml", ".map",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext));
    prefixed && !code
}

/// The licence files of a package folder, sorted by name, with those in a
/// `LICENSES` folder and the one the manifest names.
fn licence_files(dir: &Path, named: Option<&str>) -> Result<Vec<(String, PathBuf)>> {
    let mut found: BTreeMap<String, PathBuf> = BTreeMap::new();
    let entries = fs::read_dir(dir).with_context(|| format!("cannot list {}", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if path.is_file() && is_licence_file(&name) {
            found.insert(name, path);
        } else if path.is_dir() && name.eq_ignore_ascii_case("licenses") {
            for inner in fs::read_dir(&path)? {
                let inner = inner?.path();
                if inner.is_file()
                    && let Some(file) = inner.file_name().and_then(|n| n.to_str())
                {
                    found.insert(format!("{name}/{file}"), inner.clone());
                }
            }
        }
    }
    if let Some(named) = named {
        let path = dir.join(named);
        if path.is_file() {
            let label = named.replace('\\', "/");
            let already = found.values().any(|p| same_file(p, &path));
            if !already {
                found.insert(label, path);
            }
        }
    }
    Ok(found.into_iter().collect())
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// The text with Unix line ends, no trailing spaces and no blank lines at
/// either end, so the same licence written on different systems is one text.
fn normalise(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text
        .split('\n')
        .map(|line| line.trim_end_matches(['\r', ' ', '\t']))
        .collect();
    let start = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);
    let end = lines
        .iter()
        .rposition(|l| !l.is_empty())
        .map_or(0, |i| i + 1);
    lines[start..end.max(start)].join("\n")
}

fn read_texts(owner: &str, files: &[(String, PathBuf)]) -> Result<Vec<Text>> {
    let mut texts = Vec::new();
    for (label, path) in files {
        let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
        let body = normalise(&String::from_utf8_lossy(&bytes));
        if body.is_empty() {
            continue;
        }
        texts.push(Text {
            label: format!("{label} of {owner}"),
            body,
        });
    }
    Ok(texts)
}

/// The standard texts of the licences in an expression, for a package that
/// brings no licence file. Fails when one has no standard text in
/// [`STANDARD_TEXTS`].
fn standard_texts(root: &Path, title: &str, licence: Option<&str>) -> Result<Vec<Text>> {
    let Some(licence) = licence else {
        return Ok(Vec::new());
    };
    let Ok(expr) = Expr::parse(licence) else {
        return Ok(Vec::new());
    };
    let mut texts = Vec::new();
    for id in expr.licences() {
        let path = root.join(STANDARD_TEXTS).join(format!("{id}.txt"));
        let body = fs::read_to_string(&path).with_context(|| {
            format!(
                "{title} brings no licence file, and there is no standard text of {id}: add \
                 {STANDARD_TEXTS}/{id}.txt"
            )
        })?;
        texts.push(Text {
            label: format!("standard text: {id}"),
            body: normalise(&body),
        });
    }
    Ok(texts)
}

// ---------------------------------------------------------------------------
// Crates

fn crates(root: &Path) -> Result<Vec<Entry>> {
    let mut cmd = Command::new(crate::cargo());
    cmd.args(["metadata", "--format-version", "1", "--locked"]);
    for target in crate::deps::SHIPPED_TARGETS {
        cmd.args(["--filter-platform", target]);
    }
    let output = cmd
        .current_dir(root)
        .output()
        .context("cannot run `cargo metadata`")?;
    if !output.status.success() {
        bail!(
            "`cargo metadata` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let meta: Value =
        serde_json::from_slice(&output.stdout).context("cannot parse `cargo metadata`")?;
    let mut entries = Vec::new();
    for package in shipped_packages(&meta, crate::deps::SHIPPED_PACKAGES)? {
        entries.push(crate_entry(root, package)?);
    }
    entries.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(entries)
}

/// The third-party packages that the named workspace packages reach through
/// normal and build dependencies.
fn shipped_packages<'a>(meta: &'a Value, roots: &[&str]) -> Result<Vec<&'a Value>> {
    let packages: HashMap<&str, &Value> = meta["packages"]
        .as_array()
        .context("no packages")?
        .iter()
        .filter_map(|p| p["id"].as_str().map(|id| (id, p)))
        .collect();
    let mut edges: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in meta["resolve"]["nodes"]
        .as_array()
        .context("no resolve graph")?
    {
        let id = node["id"].as_str().context("node without id")?;
        let deps = node["deps"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|dep| {
                dep["dep_kinds"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|kind| kind["kind"].is_null() || kind["kind"].as_str() == Some("build"))
            })
            .filter_map(|dep| dep["pkg"].as_str())
            .collect();
        edges.insert(id, deps);
    }
    let members: BTreeSet<&str> = meta["workspace_members"]
        .as_array()
        .context("no workspace members")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let mut stack: Vec<&str> = Vec::new();
    for name in roots {
        let id = members
            .iter()
            .find(|id| packages.get(**id).and_then(|p| p["name"].as_str()) == Some(name))
            .with_context(|| format!("package `{name}` is not in the workspace"))?;
        stack.push(id);
    }
    let mut seen = BTreeSet::new();
    while let Some(id) = stack.pop() {
        if seen.insert(id) {
            stack.extend(edges.get(id).into_iter().flatten().copied());
        }
    }
    Ok(seen
        .into_iter()
        .filter(|id| !members.contains(id))
        .filter_map(|id| packages.get(id).copied())
        .collect())
}

fn crate_entry(root: &Path, package: &Value) -> Result<Entry> {
    let name = package["name"].as_str().context("crate without name")?;
    let version = package["version"]
        .as_str()
        .context("crate without version")?;
    let title = format!("{name} {version}");
    let manifest = package["manifest_path"]
        .as_str()
        .context("crate without manifest")?;
    let dir = Path::new(manifest)
        .parent()
        .context("manifest without folder")?;
    let licence = package["license"].as_str().map(str::to_owned);
    let files = licence_files(dir, package["license_file"].as_str())?;
    let mut texts = read_texts(&title, &files)?;
    let standard = texts.is_empty();
    if standard {
        texts = standard_texts(root, &title, licence.as_deref())?;
    }
    let authors: Vec<&str> = package["authors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    Ok(Entry {
        kind: Kind::Crate,
        name: name.to_owned(),
        version: Some(version.to_owned()),
        licence,
        source: package["repository"]
            .as_str()
            .or_else(|| package["homepage"].as_str())
            .map(str::to_owned),
        package: Some(format!("https://crates.io/crates/{name}/{version}")),
        used: None,
        copyright: (standard && !authors.is_empty()).then(|| authors.join(", ")),
        note: None,
        texts,
        standard,
    })
}

// ---------------------------------------------------------------------------
// npm packages

fn npm_packages(root: &Path, policy: &Policy) -> Result<Vec<Entry>> {
    let output = Command::new("pnpm")
        .args(["licenses", "list", "--prod", "--json"])
        .current_dir(root.join("ui"))
        .output()
        .context("cannot run `pnpm licenses list`")?;
    if !output.status.success() {
        bail!(
            "`pnpm licenses list` failed (are the dependencies installed?): {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let list: Value =
        serde_json::from_slice(&output.stdout).context("cannot parse `pnpm licenses list`")?;
    let mut dirs = BTreeSet::new();
    for group in list
        .as_object()
        .context("unexpected `pnpm licenses` output")?
        .values()
    {
        for package in group.as_array().into_iter().flatten() {
            for path in package["paths"].as_array().into_iter().flatten() {
                if let Some(path) = path.as_str() {
                    dirs.insert(PathBuf::from(path));
                }
            }
        }
    }
    let mut entries = Vec::new();
    for dir in dirs {
        entries.push(npm_entry(root, &dir, policy)?);
    }
    entries.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    entries.dedup();
    Ok(entries)
}

fn npm_entry(root: &Path, dir: &Path, policy: &Policy) -> Result<Entry> {
    let manifest = dir.join("package.json");
    let text = fs::read_to_string(&manifest)
        .with_context(|| format!("cannot read {}", manifest.display()))?;
    let package: Value = serde_json::from_str(&text)
        .with_context(|| format!("cannot parse {}", manifest.display()))?;
    let name = package["name"]
        .as_str()
        .context("npm package without name")?;
    let version = package["version"]
        .as_str()
        .context("npm package without version")?;
    let title = format!("{name} {version}");
    let licence = policy
        .override_for(Kind::Npm, name)
        .map(str::to_owned)
        .or_else(|| npm_licence(&package));
    let files = licence_files(dir, None)?;
    let mut texts = read_texts(&title, &files)?;
    let standard = texts.is_empty();
    if standard {
        texts = standard_texts(root, &title, licence.as_deref())?;
    }
    let author = person(&package["author"]);
    Ok(Entry {
        kind: Kind::Npm,
        name: name.to_owned(),
        version: Some(version.to_owned()),
        licence,
        source: repository(&package["repository"]).or_else(|| {
            package["homepage"]
                .as_str()
                .filter(|h| h.starts_with("https://"))
                .map(str::to_owned)
        }),
        package: Some(format!("https://www.npmjs.com/package/{name}/v/{version}")),
        used: None,
        copyright: if standard { author } else { None },
        note: None,
        texts,
        standard,
    })
}

/// The licence of a package.json: `license`, or the older `licenses` list
/// (read as a choice).
fn npm_licence(package: &Value) -> Option<String> {
    match &package["license"] {
        Value::String(s) if !s.is_empty() => return Some(s.clone()),
        Value::Object(o) => return o.get("type").and_then(Value::as_str).map(str::to_owned),
        _ => {}
    }
    let list: Vec<&str> = package["licenses"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| l["type"].as_str().or_else(|| l.as_str()))
        .collect();
    (!list.is_empty()).then(|| list.join(" OR "))
}

fn person(value: &Value) -> Option<String> {
    let text = match value {
        Value::String(s) => s.clone(),
        Value::Object(o) => o.get("name").and_then(Value::as_str)?.to_owned(),
        _ => return None,
    };
    (!text.trim().is_empty()).then(|| text.trim().to_owned())
}

/// A web address of a package.json `repository`.
fn repository(value: &Value) -> Option<String> {
    let url = match value {
        Value::String(s) => s.as_str(),
        Value::Object(o) => o.get("url").and_then(Value::as_str)?,
        _ => return None,
    };
    let url = url.strip_prefix("git+").unwrap_or(url);
    let url = url.strip_suffix(".git").unwrap_or(url);
    let url = if let Some(rest) = url.strip_prefix("git://") {
        format!("https://{rest}")
    } else if let Some(rest) = url.strip_prefix("ssh://git@") {
        format!("https://{rest}")
    } else if let Some(rest) = url.strip_prefix("github:") {
        format!("https://github.com/{rest}")
    } else if !url.contains(':') && url.matches('/').count() == 1 {
        format!("https://github.com/{url}")
    } else {
        url.to_owned()
    };
    url.starts_with("https://").then_some(url)
}

// ---------------------------------------------------------------------------
// The file

const INTRO: &str = "\
Glyphkeel is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option)
any later version. The licence is in the file LICENSE that comes with
Glyphkeel.

Glyphkeel contains the third-party software listed below. Each part is
distributed under its own licence, whose text is in the last section.
Where a part is offered under a choice of licences, all of them are shown;
Glyphkeel uses it under one that is compatible with the GNU General Public
License. The source code of each part can be had from the addresses given.

This file is generated by `cargo xtask notices` from Cargo.lock,
pnpm-lock.yaml and licences.toml. Do not change it by hand.";

/// Text indented by two spaces, with lines that don't start with a space
/// wrapped at 78 columns; indented lines (verses, examples) stay as they are.
fn indented(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        if line.is_empty() {
            out.push('\n');
            continue;
        }
        if line.starts_with(' ') {
            let _ = writeln!(out, "  {line}");
            continue;
        }
        let mut current = String::new();
        for word in line.split_whitespace() {
            if !current.is_empty() && current.len() + 1 + word.len() > 76 {
                let _ = writeln!(out, "  {current}");
                current.clear();
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        let _ = writeln!(out, "  {current}");
    }
    out
}

fn heading(out: &mut String, title: &str, underline: char) {
    let width = title.chars().count();
    let _ = writeln!(out, "{title}\n{}\n", underline.to_string().repeat(width));
}

fn render(entries: &[Entry]) -> Result<String> {
    // Texts are numbered in the order they are first used.
    let mut numbers: HashMap<&str, usize> = HashMap::new();
    let mut texts: Vec<&Text> = Vec::new();
    let mut refs: Vec<Vec<usize>> = Vec::new();
    for entry in entries {
        let mut own = Vec::new();
        for text in &entry.texts {
            let n = *numbers.entry(text.body.as_str()).or_insert_with(|| {
                texts.push(text);
                texts.len()
            });
            if !own.contains(&n) {
                own.push(n);
            }
        }
        refs.push(own);
    }

    let mut out = String::new();
    heading(&mut out, "Third-party notices for Glyphkeel", '=');
    out.push_str(INTRO);
    out.push_str("\n\n");
    let count = |kind| entries.iter().filter(|e| e.kind == kind).count();
    let sections = [
        (
            Kind::Crate,
            format!("1. Rust crates ({}), in both programs", count(Kind::Crate)),
        ),
        (
            Kind::Npm,
            format!(
                "2. JavaScript packages ({}), in the app's interface",
                count(Kind::Npm)
            ),
        ),
        (
            Kind::Component,
            format!("3. Other components ({})", count(Kind::Component)),
        ),
    ];
    out.push_str("Contents\n\n");
    for (_, title) in &sections {
        let _ = writeln!(out, "  {title}");
    }
    let _ = writeln!(out, "  4. Licence texts ({})\n\n", texts.len());

    for (kind, title) in &sections {
        heading(&mut out, title, '=');
        for (entry, own) in entries.iter().zip(&refs).filter(|(e, _)| e.kind == *kind) {
            let _ = writeln!(out, "{}", entry.title());
            if let Some(used) = &entry.used {
                out.push_str(&indented(used));
            }
            let _ = writeln!(
                out,
                "  Licence: {}",
                entry.licence.as_deref().unwrap_or("(none named)")
            );
            if let Some(source) = &entry.source {
                let _ = writeln!(out, "  Source: {source}");
            }
            if let Some(package) = &entry.package {
                let _ = writeln!(out, "  Package: {package}");
            }
            if let Some(copyright) = &entry.copyright {
                let _ = writeln!(out, "  Copyright: {copyright}");
            }
            if entry.standard {
                let _ = writeln!(
                    out,
                    "  The package has no licence file: the standard text of its licence applies."
                );
            }
            if !own.is_empty() {
                let list: Vec<String> = own.iter().map(|n| format!("[{n}]")).collect();
                let _ = writeln!(out, "  Texts: {}", list.join(", "));
            }
            if let Some(note) = &entry.note {
                out.push('\n');
                out.push_str(&indented(note));
            }
            out.push('\n');
        }
        out.push('\n');
    }

    heading(&mut out, "4. Licence texts", '=');
    for (i, text) in texts.iter().enumerate() {
        let title = format!("[{}] {}", i + 1, text.label);
        heading(&mut out, &title, '-');
        out.push_str(&text.body);
        out.push_str("\n\n\n");
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::json;

    use super::{
        Entry, Kind, Policy, Text, indented, is_licence_file, normalise, npm_licence, render,
        repository, shipped_packages,
    };

    fn entry(kind: Kind, name: &str, licence: &str) -> Entry {
        Entry {
            kind,
            name: name.into(),
            version: Some("1.0.0".into()),
            licence: Some(licence.into()),
            source: None,
            package: None,
            used: None,
            copyright: None,
            note: None,
            texts: vec![Text {
                label: format!("LICENSE of {name}"),
                body: "text".into(),
            }],
            standard: false,
        }
    }

    const POLICY: &str = r#"
allow = ["MIT", "Apache-2.0"]

[[exception]]
kind = "crate"
name = "finl_unicode"
allow = ["Unicode-DFS-2016"]
reason = "why"

[[override]]
kind = "npm"
name = "khroma"
licence = "MIT"
reason = "why"
"#;

    #[test]
    fn the_repository_policy_parses_and_matches_cargo_deny() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let policy = Policy::read(&root).unwrap();
        assert_eq!(policy.matches_deny(&root).unwrap(), Vec::<String>::new());
        assert!(policy.allow.contains("MIT"));
        assert!(!policy.allow.contains("AGPL-3.0-only"));
        assert!(!policy.allow.contains("EPL-2.0"));
    }

    #[test]
    fn a_licence_outside_the_policy_fails_and_exceptions_are_per_package() {
        let policy = Policy::parse(POLICY).unwrap();
        let entries = [
            entry(Kind::Crate, "a", "MIT OR Apache-2.0"),
            entry(Kind::Crate, "b", "EPL-2.0"),
            entry(Kind::Npm, "elk", "EPL-2.0 OR MIT"),
            entry(
                Kind::Crate,
                "finl_unicode",
                "(MIT OR Apache-2.0) AND Unicode-DFS-2016",
            ),
            entry(Kind::Crate, "other", "Unicode-DFS-2016"),
            entry(Kind::Npm, "khroma", "MIT"),
        ];
        let problems = policy.problems(&entries);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("crate b 1.0.0"));
        assert!(problems[1].contains("crate other 1.0.0"));
    }

    #[test]
    fn unneeded_exceptions_and_overrides_are_reported() {
        let policy = Policy::parse(POLICY).unwrap();
        let problems = policy.problems(&[entry(Kind::Crate, "finl_unicode", "MIT")]);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("exception for crate finl_unicode"));
        assert!(problems[1].contains("override for npm khroma"));
    }

    #[test]
    fn a_package_without_a_licence_needs_an_override() {
        let policy = Policy::parse("allow = [\"MIT\"]").unwrap();
        let mut unnamed = entry(Kind::Npm, "x", "MIT");
        unnamed.licence = None;
        let problems = policy.problems(&[unnamed]);
        assert!(problems[0].contains("names no licence"), "{problems:?}");
    }

    #[test]
    fn the_allow_lists_must_match() {
        let policy = Policy::parse(POLICY).unwrap();
        let deny: BTreeSet<String> = ["MIT".to_owned(), "ISC".to_owned()].into();
        let problems = policy.compare_allow(&deny);
        assert_eq!(problems.len(), 2);
        assert!(problems[0].ends_with("Apache-2.0"));
        assert!(problems[1].ends_with("ISC"));
    }

    #[test]
    fn policy_entries_need_their_fields() {
        assert!(Policy::parse("allow = []").is_err());
        assert!(
            Policy::parse("allow = [\"MIT\"]\n[[exception]]\nkind = \"crate\"\nname = \"x\"\n")
                .is_err(),
            "an exception without a reason"
        );
        assert!(
            Policy::parse(
                "allow = [\"MIT\"]\n[[override]]\nkind = \"pip\"\nname = \"x\"\nlicence = \"MIT\"\nreason = \"r\"\n"
            )
            .is_err(),
            "an unknown kind"
        );
    }

    #[test]
    fn licence_files_are_recognised_by_name() {
        for name in [
            "LICENSE",
            "LICENSE-MIT",
            "license.md",
            "LICENCE.txt",
            "COPYING",
            "NOTICE",
            "UNLICENSE",
            "COPYRIGHT",
        ] {
            assert!(is_licence_file(name), "{name}");
        }
        for name in [
            "README.md",
            "license.rs",
            "license.json",
            "src",
            "THIRD-PARTY-LICENSE",
        ] {
            assert!(!is_licence_file(name), "{name}");
        }
    }

    #[test]
    fn notes_are_indented_and_wrapped() {
        let long = "word ".repeat(30);
        let text = indented(&format!("{long}\n\n    a verse"));
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.iter().all(|l| l.len() <= 78), "{lines:?}");
        assert!(lines[0].starts_with("  word"));
        assert!(lines.contains(&""));
        assert!(text.ends_with("      a verse\n"));
    }

    #[test]
    fn texts_are_normalised() {
        assert_eq!(normalise("\u{feff}\r\n\nA  \r\nB\t\n\n\n"), "A\nB");
        assert_eq!(normalise("\n\n"), "");
    }

    #[test]
    fn npm_licences_and_repositories_are_read() {
        assert_eq!(
            npm_licence(&json!({"license": "MIT"})).as_deref(),
            Some("MIT")
        );
        assert_eq!(
            npm_licence(&json!({"licenses": [{"type": "MIT"}, {"type": "Apache-2.0"}]})).as_deref(),
            Some("MIT OR Apache-2.0")
        );
        assert_eq!(npm_licence(&json!({})), None);
        assert_eq!(
            repository(&json!({"url": "git+https://github.com/a/b.git"})).as_deref(),
            Some("https://github.com/a/b")
        );
        assert_eq!(
            repository(&json!("git://github.com/a/b.git")).as_deref(),
            Some("https://github.com/a/b")
        );
        assert_eq!(
            repository(&json!("github:a/b")).as_deref(),
            Some("https://github.com/a/b")
        );
        assert_eq!(
            repository(&json!("a/b")).as_deref(),
            Some("https://github.com/a/b")
        );
        assert_eq!(repository(&json!("http://example.org/x")), None);
    }

    #[test]
    fn shipped_packages_follow_normal_and_build_dependencies_only() {
        let normal = json!([{ "kind": null }]);
        let build = json!([{ "kind": "build" }]);
        let dev = json!([{ "kind": "dev" }]);
        let meta = json!({
            "packages": [
                { "id": "app", "name": "gk-app" },
                { "id": "x", "name": "x" },
                { "id": "y", "name": "y" },
                { "id": "z", "name": "z" },
                { "id": "w", "name": "w" },
            ],
            "workspace_members": ["app"],
            "resolve": { "nodes": [
                { "id": "app", "deps": [
                    { "pkg": "x", "dep_kinds": normal },
                    { "pkg": "y", "dep_kinds": build },
                    { "pkg": "z", "dep_kinds": dev },
                ]},
                { "id": "x", "deps": [{ "pkg": "w", "dep_kinds": normal }] },
                { "id": "y", "deps": [] },
                { "id": "z", "deps": [] },
                { "id": "w", "deps": [] },
            ]},
        });
        let names: Vec<&str> = shipped_packages(&meta, &["gk-app"])
            .unwrap()
            .iter()
            .filter_map(|p| p["name"].as_str())
            .collect();
        assert_eq!(names, ["w", "x", "y"]);
        assert!(shipped_packages(&meta, &["gk-cli"]).is_err());
    }

    #[test]
    fn the_same_text_is_printed_once_and_numbered_in_order() {
        let mut a = entry(Kind::Crate, "a", "MIT");
        a.texts[0].body = "shared".into();
        let mut b = entry(Kind::Npm, "b", "MIT");
        b.texts = vec![
            Text {
                label: "LICENSE-APACHE of b".into(),
                body: "apache".into(),
            },
            Text {
                label: "LICENSE of b".into(),
                body: "shared".into(),
            },
        ];
        let text = render(&[a, b]).unwrap();
        assert_eq!(text.matches("\nshared\n").count(), 1);
        assert!(text.contains("a 1.0.0\n  Licence: MIT\n  Texts: [1]\n"));
        assert!(text.contains("b 1.0.0\n  Licence: MIT\n  Texts: [2], [1]\n"));
        assert!(text.contains("[1] LICENSE of a"));
        assert!(text.contains("[2] LICENSE-APACHE of b"));
        assert!(text.ends_with("apache\n"));
    }
}
