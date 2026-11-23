// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Project keys (`TM`) and task keys (`TM-41`).
//!
//! A task's key is its project's key, a hyphen and the task's number in that
//! project. Numbers are handed out from the project's counter, which only
//! grows: a number is never given out twice, not even after its task was
//! deleted and purged. A task moved to another project gets the next number
//! there; renamed project keys and moved tasks leave aliases behind, so old
//! keys keep resolving.

use std::fmt;
use std::str::FromStr;

/// Fewest characters in a project key.
pub const PROJECT_KEY_MIN: usize = 2;
/// Most characters in a project key.
pub const PROJECT_KEY_MAX: usize = 10;

/// A project key: 2 to 10 characters of `A`–`Z` and `0`–`9`, starting with a
/// letter.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectKey(String);

/// A task key: a project key and a task number of at least 1.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskKey {
    project: ProjectKey,
    number: u32,
}

/// Why text is not a valid key. The messages say what a valid key looks like.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    /// Wrong length.
    #[error(
        "a project key has {PROJECT_KEY_MIN} to {PROJECT_KEY_MAX} characters; \"{key}\" has {len}"
    )]
    Length {
        /// The rejected key.
        key: String,
        /// Its length in characters.
        len: usize,
    },
    /// Starts with something other than a letter.
    #[error("a project key starts with a letter A–Z; \"{0}\" does not")]
    FirstNotLetter(String),
    /// Contains a character other than `A`–`Z` and `0`–`9`.
    #[error(
        "a project key uses only the capital letters A–Z and the digits 0–9; \"{key}\" contains '{ch}'"
    )]
    Character {
        /// The rejected key.
        key: String,
        /// The first character that is not allowed.
        ch: char,
    },
    /// The task key has no `-<number>` part.
    #[error("\"{0}\" is not a task key: expected a project key, a hyphen and a number, like TM-41")]
    TaskKeyShape(String),
    /// The number is zero, too large or not a number.
    #[error("\"{0}\" is not a task number: expected a whole number from 1")]
    Number(String),
}

impl ProjectKey {
    /// Checks `key` exactly as typed: lower-case letters are rejected, so the
    /// user sees the key that will be stored.
    pub fn new(key: &str) -> Result<Self, KeyError> {
        let len = key.chars().count();
        if !(PROJECT_KEY_MIN..=PROJECT_KEY_MAX).contains(&len) {
            return Err(KeyError::Length {
                key: key.to_owned(),
                len,
            });
        }
        if let Some(ch) = key
            .chars()
            .find(|c| !(c.is_ascii_uppercase() || c.is_ascii_digit()))
        {
            return Err(KeyError::Character {
                key: key.to_owned(),
                ch,
            });
        }
        if !key.starts_with(|c: char| c.is_ascii_uppercase()) {
            return Err(KeyError::FirstNotLetter(key.to_owned()));
        }
        Ok(Self(key.to_owned()))
    }

    /// Like [`ProjectKey::new`], but upper-cases ASCII letters first. For
    /// resolving keys that people type in references and commands (`tm-41`).
    pub fn parse_lenient(key: &str) -> Result<Self, KeyError> {
        Self::new(&key.trim().to_ascii_uppercase())
    }

    /// The key as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TaskKey {
    /// A task key from its parts. `number` must be at least 1.
    pub fn new(project: ProjectKey, number: u32) -> Result<Self, KeyError> {
        if number == 0 {
            return Err(KeyError::Number("0".to_owned()));
        }
        Ok(Self { project, number })
    }

    /// The project part.
    #[must_use]
    pub fn project(&self) -> &ProjectKey {
        &self.project
    }

    /// The number part.
    #[must_use]
    pub fn number(&self) -> u32 {
        self.number
    }
}

impl FromStr for TaskKey {
    type Err = KeyError;

    /// Parses `TM-41`; letters may be in either case and surrounding spaces
    /// are ignored. Leading zeros in the number are rejected, so every task
    /// has exactly one spelling.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        let (project, number) = trimmed
            .rsplit_once('-')
            .ok_or_else(|| KeyError::TaskKeyShape(s.to_owned()))?;
        if project.is_empty() || number.is_empty() {
            return Err(KeyError::TaskKeyShape(s.to_owned()));
        }
        let project = ProjectKey::parse_lenient(project)?;
        if !number.bytes().all(|b| b.is_ascii_digit()) || number.starts_with('0') {
            return Err(KeyError::Number(number.to_owned()));
        }
        let number: u32 = number
            .parse()
            .map_err(|_| KeyError::Number(number.to_owned()))?;
        Self::new(project, number)
    }
}

impl FromStr for ProjectKey {
    type Err = KeyError;

    /// Strict parsing, as [`ProjectKey::new`].
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl fmt::Display for ProjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for ProjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ProjectKey({})", self.0)
    }
}

impl fmt::Display for TaskKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.project, self.number)
    }
}

impl fmt::Debug for TaskKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TaskKey({self})")
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn accepts_valid_project_keys() {
        for key in ["TM", "DEMO", "A1", "ABCDEFGHIJ", "X2024"] {
            assert_eq!(ProjectKey::new(key).unwrap().as_str(), key);
        }
    }

    #[test]
    fn rejects_invalid_project_keys_with_the_reason() {
        assert!(matches!(
            ProjectKey::new("T"),
            Err(KeyError::Length { len: 1, .. })
        ));
        assert!(matches!(
            ProjectKey::new("ABCDEFGHIJK"),
            Err(KeyError::Length { len: 11, .. })
        ));
        assert!(matches!(
            ProjectKey::new("1TM"),
            Err(KeyError::FirstNotLetter(_))
        ));
        assert!(matches!(
            ProjectKey::new("tm"),
            Err(KeyError::Character { ch: 't', .. })
        ));
        assert!(matches!(
            ProjectKey::new("T-M"),
            Err(KeyError::Character { ch: '-', .. })
        ));
        assert!(matches!(
            ProjectKey::new("ÄB"),
            Err(KeyError::Character { ch: 'Ä', .. })
        ));
        let message = ProjectKey::new("t").unwrap_err().to_string();
        assert!(message.contains("2 to 10"), "{message}");
    }

    #[test]
    fn parses_task_keys() {
        let key: TaskKey = "TM-41".parse().unwrap();
        assert_eq!(key.project().as_str(), "TM");
        assert_eq!(key.number(), 41);
        assert_eq!(key.to_string(), "TM-41");
        assert_eq!(" tm-41 ".parse::<TaskKey>().unwrap(), key);
    }

    #[test]
    fn rejects_malformed_task_keys() {
        for bad in [
            "TM",
            "TM-",
            "-41",
            "TM-0",
            "TM-041",
            "TM-4a",
            "TM-+4",
            "TM-99999999999",
            "1X-3",
        ] {
            assert!(bad.parse::<TaskKey>().is_err(), "{bad}");
        }
    }

    proptest! {
        #[test]
        fn task_keys_round_trip(project in "[A-Z][A-Z0-9]{1,9}", number in 1u32..) {
            let key = TaskKey::new(ProjectKey::new(&project).unwrap(), number).unwrap();
            prop_assert_eq!(key.to_string().parse::<TaskKey>().unwrap(), key);
        }
    }
}
