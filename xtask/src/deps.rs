// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask check-deps`: the dependency rules between our crates.
//!
//! - Only the desktop app links the desktop toolkit and the web view. The
//!   command line (which also serves MCP) and every library stay lean, so they
//!   build and run without a display.
//! - Neither shipped program links an HTTP client: the app never goes online.
//!   The one HTTP crate allowed is hyper as a server, for the opt-in local
//!   endpoint for agents (127.0.0.1 only); its client features stay off.
//! - Neither shipped program links a keyring or secret-store client: the app
//!   never touches the keyring.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::Value;

/// Crates that only the desktop app may link.
const DESKTOP_ONLY: &[&str] = &[
    "tauri",
    "tao",
    "wry",
    "webkit2gtk",
    "gtk",
    "javascriptcore-rs",
];

/// HTTP clients and other network stacks, banned from both shipped programs.
const NETWORK: &[&str] = &[
    "reqwest",
    "ureq",
    "curl",
    "isahc",
    "attohttpc",
    "surf",
    "h2",
    "rustls",
    "native-tls",
    "openssl",
];

/// HTTP crates the shipped programs may link as a server only, with the
/// features that would make them a client.
const SERVER_ONLY: &[(&str, &[&str])] = &[
    ("hyper", &["client"]),
    ("hyper-util", &["client", "client-legacy", "client-proxy"]),
];

/// Keyring and secret-store clients, banned from both shipped programs.
const KEYRING: &[&str] = &[
    "keyring",
    "secret-service",
    "dbus-secret-service",
    "oo7",
    "libsecret",
    "libsecret-sys",
    "security-framework",
    "windows-credentials",
];

/// The targets the programs ship for. Dependencies that exist only on other
/// targets (Tauri's mobile code, for example) never reach a release.
pub(crate) const SHIPPED_TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
];

/// The packages that may link the desktop toolkit.
const DESKTOP_PACKAGES: &[&str] = &["gk-app"];

/// The packages that build the shipped programs.
pub(crate) const SHIPPED_PACKAGES: &[&str] = &["gk-cli", "gk-app"];

pub(crate) fn run(root: &Path) -> Result<bool> {
    let graph = Graph::load(root)?;
    let mut ok = true;
    for member in &graph.members {
        if DESKTOP_PACKAGES.contains(&member.as_str()) || member == "xtask" {
            continue;
        }
        ok &= report(
            member,
            "the desktop toolkit or web view",
            &graph.banned_in(member, DESKTOP_ONLY)?,
        );
    }
    for program in SHIPPED_PACKAGES {
        ok &= report(
            program,
            "an HTTP client",
            &graph.banned_in(program, NETWORK)?,
        );
        ok &= report(
            program,
            "a keyring client",
            &graph.banned_in(program, KEYRING)?,
        );
        ok &= report(
            program,
            "an HTTP client feature",
            &client_features(root, program)?,
        );
    }
    if ok {
        eprintln!("check-deps: the dependency rules hold");
    }
    Ok(ok)
}

fn report(package: &str, what: &str, found: &BTreeSet<String>) -> bool {
    if found.is_empty() {
        return true;
    }
    let list: Vec<&str> = found.iter().map(String::as_str).collect();
    eprintln!("{package} links {what}: {}", list.join(", "));
    false
}

/// The resolved graph of normal (not dev or build) dependencies.
struct Graph {
    members: Vec<String>,
    /// Package id to its name.
    names: HashMap<String, String>,
    /// Package id to the ids of its normal dependencies.
    edges: HashMap<String, Vec<String>>,
}

impl Graph {
    fn load(root: &Path) -> Result<Self> {
        let mut cmd = Command::new(crate::cargo());
        cmd.args(["metadata", "--format-version", "1", "--locked"]);
        for target in SHIPPED_TARGETS {
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
        Self::from_metadata(&meta)
    }

    fn from_metadata(meta: &Value) -> Result<Self> {
        let mut names = HashMap::new();
        for package in meta["packages"].as_array().context("no packages")? {
            if let (Some(id), Some(name)) = (package["id"].as_str(), package["name"].as_str()) {
                names.insert(id.to_owned(), name.to_owned());
            }
        }
        let members = meta["workspace_members"]
            .as_array()
            .context("no workspace members")?
            .iter()
            .filter_map(|id| id.as_str().and_then(|id| names.get(id)).cloned())
            .collect();
        let mut edges = HashMap::new();
        for node in meta["resolve"]["nodes"]
            .as_array()
            .context("no resolve graph")?
        {
            let id = node["id"].as_str().context("node without id")?.to_owned();
            let deps = node["deps"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|dep| is_normal(dep))
                .filter_map(|dep| dep["pkg"].as_str().map(str::to_owned))
                .collect();
            edges.insert(id, deps);
        }
        Ok(Self {
            members,
            names,
            edges,
        })
    }

    /// The names from `banned` that `package` reaches through normal
    /// dependencies, on any of the shipped targets.
    fn banned_in(&self, package: &str, banned: &[&str]) -> Result<BTreeSet<String>> {
        let start = self
            .names
            .iter()
            .find(|(id, name)| name.as_str() == package && self.edges.contains_key(*id))
            .map(|(id, _)| id.clone())
            .with_context(|| format!("package `{package}` is not in the workspace"))?;
        let mut seen = BTreeSet::new();
        let mut stack = vec![start];
        let mut found = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(name) = self.names.get(&id)
                && banned.contains(&name.as_str())
            {
                found.insert(name.clone());
            }
            stack.extend(self.edges.get(&id).into_iter().flatten().cloned());
        }
        Ok(found)
    }
}

/// The client features of the [`SERVER_ONLY`] crates that `program` builds
/// with, on any shipped target, as `crate/feature`. `cargo tree` resolves
/// the features exactly as a build does; `cargo metadata` would mix in the
/// features of dev-dependencies.
fn client_features(root: &Path, program: &str) -> Result<BTreeSet<String>> {
    let mut found = BTreeSet::new();
    for target in SHIPPED_TARGETS {
        for (krate, banned) in SERVER_ONLY {
            let output = Command::new(crate::cargo())
                .args(["tree", "--locked", "-p", program, "-e", "normal,features"])
                .args(["--target", target, "-i", krate, "--prefix", "none"])
                .current_dir(root)
                .output()
                .context("cannot run `cargo tree`")?;
            if !output.status.success() {
                // The program doesn't reach the crate on this target.
                continue;
            }
            found.extend(features_named(
                &String::from_utf8_lossy(&output.stdout),
                krate,
                banned,
            ));
        }
    }
    Ok(found)
}

/// The features of `krate` from `banned` in the output of `cargo tree -e
/// features -i <krate> --prefix none`, as `crate/feature`.
fn features_named(tree: &str, krate: &str, banned: &[&str]) -> BTreeSet<String> {
    let prefix = format!("{krate} feature \"");
    tree.lines()
        .filter_map(|line| line.trim().strip_prefix(&prefix))
        .filter_map(|rest| rest.split('"').next())
        .filter(|feature| banned.contains(feature))
        .map(|feature| format!("{krate}/{feature}"))
        .collect()
}

/// A dependency edge is normal when one of its kinds is neither `dev` nor
/// `build` (cargo writes `null` for normal).
fn is_normal(dep: &Value) -> bool {
    dep["dep_kinds"]
        .as_array()
        .is_some_and(|kinds| kinds.iter().any(|kind| kind["kind"].is_null()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use std::path::Path;

    use super::{Graph, KEYRING, features_named};

    fn graph() -> Graph {
        let normal = json!([{ "kind": null }]);
        let dev = json!([{ "kind": "dev" }]);
        Graph::from_metadata(&json!({
            "packages": [
                { "id": "cli", "name": "gk-cli" },
                { "id": "app", "name": "gk-app" },
                { "id": "core", "name": "gk-core" },
                { "id": "tauri", "name": "tauri" },
                { "id": "hyper", "name": "hyper" },
            ],
            "workspace_members": ["cli", "app", "core"],
            "resolve": { "nodes": [
                { "id": "cli", "deps": [
                    { "pkg": "core", "dep_kinds": normal },
                    { "pkg": "hyper", "dep_kinds": dev },
                ] },
                { "id": "app", "deps": [{ "pkg": "tauri", "dep_kinds": normal }] },
                { "id": "core", "deps": [] },
                { "id": "tauri", "deps": [{ "pkg": "hyper", "dep_kinds": normal }] },
                { "id": "hyper", "deps": [] },
            ] },
        }))
        .unwrap()
    }

    #[test]
    fn finds_banned_crates_through_normal_dependencies() {
        let graph = graph();
        let found = graph.banned_in("gk-app", &["hyper", "tauri"]).unwrap();
        assert_eq!(found.into_iter().collect::<Vec<_>>(), ["hyper", "tauri"]);
    }

    #[test]
    fn the_keyring_clients_are_banned() {
        for name in ["keyring", "secret-service", "oo7", "libsecret"] {
            assert!(KEYRING.contains(&name), "{name} is not banned");
        }
        let deny =
            std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../deny.toml"))
                .unwrap();
        for name in KEYRING {
            assert!(
                deny.contains(&format!("crate = \"{name}\"")),
                "deny.toml does not ban {name}"
            );
        }
    }

    #[test]
    fn ignores_dev_dependencies() {
        let graph = graph();
        assert!(
            graph
                .banned_in("gk-cli", &["hyper", "tauri"])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_server_only_crate_with_a_client_feature_is_found() {
        let tree = "hyper v1.12.0\nhyper feature \"default\"\nhyper feature \"server\"\nhyper feature \"client\"\n";
        let found = features_named(tree, "hyper", &["client"]);
        assert_eq!(found.into_iter().collect::<Vec<_>>(), ["hyper/client"]);
        let server = "hyper v1.12.0\nhyper feature \"http1\"\nhyper feature \"server\"\n";
        assert!(features_named(server, "hyper", &["client"]).is_empty());
    }

    #[test]
    fn unknown_package_is_an_error() {
        assert!(graph().banned_in("gk-nothing", &["tauri"]).is_err());
    }
}
