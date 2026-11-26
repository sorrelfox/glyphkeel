// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Where a column of a CSV file can go.

use gk_api::ops::task::RelationKind;

/// A column's target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    Title,
    Description,
    Status,
    Type,
    Priority,
    Labels,
    /// The version the task is planned for, by name in the project.
    FixVersion,
    StartDate,
    DueDate,
    Estimate,
    Spent,
    Comment,
    /// The row's key or ID in the other tool.
    Key,
    Parent,
    /// One kind of link, to the keys the column holds.
    Link(RelationKind),
    /// Redmine's "Related issues".
    Relations,
    /// A custom field, by name.
    Field(String),
    Ignore,
}

impl Target {
    /// The target `text` names.
    pub(crate) fn parse(text: &str) -> Result<Self, ()> {
        let t = text.trim();
        if let Some(name) = t.strip_prefix("field.") {
            let name = name.trim();
            return if name.is_empty() {
                Err(())
            } else {
                Ok(Self::Field(name.to_owned()))
            };
        }
        Ok(match t {
            "title" => Self::Title,
            "description" => Self::Description,
            "status" => Self::Status,
            "type" => Self::Type,
            "priority" => Self::Priority,
            "labels" => Self::Labels,
            "fix_version" => Self::FixVersion,
            "start_date" => Self::StartDate,
            "due_date" => Self::DueDate,
            "estimate" => Self::Estimate,
            "spent" => Self::Spent,
            "comment" => Self::Comment,
            "key" => Self::Key,
            "parent" => Self::Parent,
            "blocks" => Self::Link(RelationKind::Blocks),
            "blocked_by" => Self::Link(RelationKind::BlockedBy),
            "relates" => Self::Link(RelationKind::Relates),
            "duplicates" => Self::Link(RelationKind::Duplicates),
            "duplicated_by" => Self::Link(RelationKind::DuplicatedBy),
            "clones" => Self::Link(RelationKind::Clones),
            "cloned_by" => Self::Link(RelationKind::ClonedBy),
            "relations" => Self::Relations,
            "ignore" | "" => Self::Ignore,
            _ => return Err(()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_by_name() {
        assert_eq!(Target::parse("title"), Ok(Target::Title));
        assert_eq!(
            Target::parse("field.Story points"),
            Ok(Target::Field("Story points".to_owned()))
        );
        assert_eq!(
            Target::parse("blocked_by"),
            Ok(Target::Link(RelationKind::BlockedBy))
        );
        assert_eq!(Target::parse("fix_version"), Ok(Target::FixVersion));
        assert_eq!(Target::parse("field."), Err(()));
        assert_eq!(Target::parse("assignee"), Err(()));
    }
}
