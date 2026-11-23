// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Operation errors as MCP tool results.
//!
//! An error comes back as a tool result with `isError: true` and one text
//! block, so the agent can read it and correct itself. It carries no
//! `structuredContent`: clients validate that against the tool's output
//! schema, which describes a success.

use gk_api::OpError;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::Value;

/// The error as an `isError` result with text only.
#[must_use]
pub(crate) fn tool_error(error: &OpError) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(error_text(error))])
}

/// The error's text: its code, the message, the hint, then the details an
/// agent can act on, one per line.
#[must_use]
pub fn error_text(error: &OpError) -> String {
    let mut text = format!("Error ({}): {}", error.code.as_str(), error.message);
    if let Some(hint) = &error.hint {
        text.push(' ');
        text.push_str(hint);
    }
    for (key, value) in &error.details {
        let value = match value {
            Value::String(s) => s.clone(),
            Value::Array(items) => items
                .iter()
                .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned))
                .collect::<Vec<_>>()
                .join(", "),
            other => other.to_string(),
        };
        let label = match key.as_str() {
            "path" => "Field",
            "choices" => "Valid",
            "retry_after" => "Retry after (s)",
            "max" => "Most allowed",
            "min" => "Least allowed",
            other => other,
        };
        text.push_str(&format!("\n{label}: {value}"));
    }
    text
}

#[cfg(test)]
mod tests {
    use gk_api::ErrorCode;

    use super::*;

    #[test]
    fn the_text_names_what_was_wrong_and_what_is_valid() {
        let e = OpError::not_found("Status 'Doing' doesn't exist in TM.", &["To do", "Done"])
            .with_hint("Use one of the listed statuses.");
        assert_eq!(
            error_text(&e),
            "Error (not_found): Status 'Doing' doesn't exist in TM. Use one of the listed \
             statuses.\nValid: To do, Done"
        );
    }

    #[test]
    fn error_results_carry_text_only() {
        let r = tool_error(&OpError::new(ErrorCode::Invalid, "Bad.").with_detail("path", "limit"));
        assert_eq!(r.is_error, Some(true));
        assert!(r.structured_content.is_none());
        assert_eq!(r.content.len(), 1);
        let text = r.content[0].as_text().map(|t| t.text.clone());
        assert_eq!(text.as_deref(), Some("Error (invalid): Bad.\nField: limit"));
    }
}
