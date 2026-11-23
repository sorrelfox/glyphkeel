// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The one error type every operation returns, in every adapter.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// What kind of failure an operation error is. Adapters decide on the code, never
/// on the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The input doesn't parse or breaks a rule; `details.path` names the
    /// field when there is one.
    Invalid,
    /// No such item or name; `details.choices` lists the valid ones when the
    /// list is short.
    NotFound,
    /// The item changed since the caller read it, or the change collides
    /// with a newer one.
    Conflict,
    /// The caller's profile doesn't allow this, the operation needs a
    /// confirmation that wasn't given, or the item is archived.
    Forbidden,
    /// A size limit, a cap on bulk changes or a rate limit.
    Limit,
    /// Another program kept the workspace locked; trying again later works.
    Busy,
    /// A newer program upgraded the workspace while this one was running.
    /// The program must restart.
    WorkspaceUpgraded,
    /// The workspace was made by a newer program.
    WorkspaceTooNew,
    /// The disk, the file system or the database failed.
    Storage,
}

impl ErrorCode {
    /// The exit code of the `glyphkeel` command for this error: 1 invalid
    /// input, 2 not found, 3 change rejected, 4 workspace or storage problem.
    #[must_use]
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Invalid => 1,
            Self::NotFound => 2,
            Self::Conflict | Self::Forbidden | Self::Limit => 3,
            Self::Busy | Self::WorkspaceUpgraded | Self::WorkspaceTooNew | Self::Storage => 4,
        }
    }

    /// The code as written in JSON.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "invalid",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Forbidden => "forbidden",
            Self::Limit => "limit",
            Self::Busy => "busy",
            Self::WorkspaceUpgraded => "workspace_upgraded",
            Self::WorkspaceTooNew => "workspace_too_new",
            Self::Storage => "storage",
        }
    }
}

/// An operation failed. The message says what happened, the hint what to do
/// next; both are written for the reader, person or agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpError {
    /// What kind of failure.
    pub code: ErrorCode,
    /// What happened.
    pub message: String,
    /// What to do next.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Facts a program can act on: `path`, `choices`, `current_version`,
    /// `max`, `retry_after`, …
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub details: Map<String, Value>,
}

impl OpError {
    /// An error with a code and a message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            hint: None,
            details: Map::new(),
        }
    }

    /// `invalid`.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Invalid, message)
    }

    /// `invalid` for one field, with its JSON path in `details.path`.
    pub fn invalid_at(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::invalid(message).with_detail("path", path.into())
    }

    /// `not_found`, listing the valid choices in `details.choices`.
    pub fn not_found(message: impl Into<String>, choices: &[&str]) -> Self {
        let error = Self::new(ErrorCode::NotFound, message);
        if choices.is_empty() {
            error
        } else {
            error.with_detail("choices", choices.to_vec())
        }
    }

    /// `conflict`.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message)
    }

    /// `forbidden`.
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }

    /// `limit`, with the maximum in `details.max`.
    pub fn limit(message: impl Into<String>, max: u64) -> Self {
        Self::new(ErrorCode::Limit, message).with_detail("max", max)
    }

    /// `storage`.
    pub fn storage(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Storage, message)
    }

    /// Adds a hint.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Adds one entry to `details`.
    #[must_use]
    pub fn with_detail(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.details.insert(key.to_owned(), value.into());
        self
    }
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, " {hint}")?;
        }
        Ok(())
    }
}

impl std::error::Error for OpError {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn absent_hint_and_details_are_left_out() {
        let e = OpError::conflict("TM-41 changed since you read it.");
        assert_eq!(
            serde_json::to_value(&e).expect("serializes"),
            json!({ "code": "conflict", "message": "TM-41 changed since you read it." })
        );
    }

    #[test]
    fn not_found_lists_choices() {
        let e = OpError::not_found("No status 'Doing'.", &["To do", "Done"])
            .with_hint("Use one of the listed statuses.");
        assert_eq!(e.details["choices"], json!(["To do", "Done"]));
        assert_eq!(e.code.exit_code(), 2);
        assert_eq!(
            e.to_string(),
            "No status 'Doing'. Use one of the listed statuses."
        );
    }

    #[test]
    fn codes_serialize_as_their_names() {
        for code in [
            ErrorCode::Invalid,
            ErrorCode::NotFound,
            ErrorCode::Conflict,
            ErrorCode::Forbidden,
            ErrorCode::Limit,
            ErrorCode::Busy,
            ErrorCode::WorkspaceUpgraded,
            ErrorCode::WorkspaceTooNew,
            ErrorCode::Storage,
        ] {
            assert_eq!(
                serde_json::to_value(code).expect("serializes"),
                json!(code.as_str())
            );
        }
    }
}
