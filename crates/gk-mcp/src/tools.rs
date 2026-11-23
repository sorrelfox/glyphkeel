// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The tool list, built from the operation catalog's tools in their order.
//!
//! Each tool's definition is exactly what the catalog's golden tool list
//! holds for it (name, title, description, portable schemas, annotations),
//! so the list served and the checked-in list can't drift apart.

use std::sync::OnceLock;

use gk_api::mcp::TOOLS;
use gk_api::{McpTool, Profile, Toolset};
use rmcp::model::Tool;

/// Every catalog tool with its MCP definition, in catalog order. Built on
/// first use.
fn all() -> &'static [(McpTool, Tool)] {
    static ALL: OnceLock<Vec<(McpTool, Tool)>> = OnceLock::new();
    ALL.get_or_init(|| {
        TOOLS
            .iter()
            .map(|t| {
                let tool = serde_json::from_value(t.to_json()).unwrap_or_else(|e| {
                    panic!("the definition of {} doesn't fit MCP: {e}", t.name)
                });
                (*t, tool)
            })
            .collect()
    })
}

/// The tools an agent with the effective `profile` sees with `toolsets`, in
/// catalog order.
pub(crate) fn listed(
    profile: Profile,
    toolsets: &[Toolset],
) -> impl Iterator<Item = &'static (McpTool, Tool)> {
    all()
        .iter()
        .filter(move |(t, _)| toolsets.contains(&t.toolset) && t.allowed_for(profile))
}

/// The tool named `name`, if the catalog has one, whether listed or not.
#[must_use]
pub(crate) fn find(name: &str) -> Option<&'static (McpTool, Tool)> {
    all().iter().find(|(t, _)| t.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_tool_has_a_definition_in_order() {
        let names: Vec<&str> = all().iter().map(|(t, _)| t.name).collect();
        let catalog: Vec<&str> = TOOLS.iter().map(|t| t.name).collect();
        assert_eq!(names, catalog);
        for (t, tool) in all() {
            assert_eq!(tool.name, t.name);
            assert!(
                tool.output_schema.is_some(),
                "{} has an output schema",
                t.name
            );
            let annotations = tool.annotations.as_ref().expect("annotations");
            assert_eq!(annotations.open_world_hint, Some(false));
        }
    }
}
