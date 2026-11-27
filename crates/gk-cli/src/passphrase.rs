// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The passphrase of an encrypted workspace on the command line: from the
//! environment (`GLYPHKEEL_PASSPHRASE`), or asked on the terminal without
//! echo. Without a terminal, nothing is asked and the
//! command fails with a hint.

use std::io::{BufRead, IsTerminal};

use gk_api::OpError;
use gk_store::{Passphrase, StoreError, WorkspaceLayout};

use crate::messages::{self, text};

/// Tries before the command gives up.
const TRIES: usize = 3;

fn terminal() -> bool {
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// Makes the workspace at `layout` openable: a plain one needs nothing; an
/// encrypted one needs a passphrase from the environment or the person at
/// the terminal.
pub(crate) fn unlock(layout: &WorkspaceLayout) -> Result<(), OpError> {
    let database = layout.database();
    let missing = match gk_store::unlock_without_asking(&database) {
        Ok(_) => return Ok(()),
        Err(e @ (StoreError::PassphraseRequired(_) | StoreError::WrongPassphrase(_))) => e,
        Err(e) => return Err(gk_engine::store_error(&e)),
    };
    if !terminal() {
        return Err(gk_engine::store_error(&missing));
    }
    let name = gk_engine::workspaces::name_of(layout);
    for _ in 0..TRIES {
        let Some(passphrase) = ask(&text("cli-passphrase-prompt", &[("name", name.clone())]))?
        else {
            break;
        };
        if gk_store::passphrase_opens(&database, &passphrase)
            .map_err(|e| gk_engine::store_error(&e))?
        {
            return Ok(());
        }
        eprintln!("{}", text("cli-passphrase-wrong", &[]));
    }
    Err(gk_engine::store_error(&StoreError::WrongPassphrase(
        database,
    )))
}

/// A new passphrase: asked twice on the terminal, else read as one line
/// from standard input, so scripts can pipe it in.
pub(crate) fn new_passphrase() -> Result<Passphrase, OpError> {
    let empty = || {
        messages::hint(
            messages::error("cli-passphrase-empty", &[], OpError::invalid),
            "cli-passphrase-empty-hint",
            &[],
        )
    };
    if terminal() {
        let first = ask(&text("cli-passphrase-new", &[]))?.ok_or_else(empty)?;
        let again = ask(&text("cli-passphrase-repeat", &[]))?.ok_or_else(empty)?;
        if first != again {
            return Err(messages::error(
                "cli-passphrase-mismatch",
                &[],
                OpError::invalid,
            ));
        }
        return Ok(first);
    }
    let mut line = zeroize::Zeroizing::new(String::new());
    std::io::stdin().lock().read_line(&mut line).map_err(|e| {
        messages::error(
            "cli-passphrase-unreadable",
            &[("error", e.to_string())],
            OpError::invalid,
        )
    })?;
    let text = line.trim_end_matches(['\r', '\n']);
    Passphrase::new(text).ok_or_else(empty)
}

/// Asks for a passphrase on the terminal without echo; `None` when the
/// answer is empty.
fn ask(prompt: &str) -> Result<Option<Passphrase>, OpError> {
    let answer = rpassword::prompt_password(format!("{prompt} ")).map_err(|e| {
        messages::error(
            "cli-passphrase-unreadable",
            &[("error", e.to_string())],
            OpError::invalid,
        )
    })?;
    Ok(Passphrase::new(answer))
}
