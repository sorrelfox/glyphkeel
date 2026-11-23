// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Agent profiles: which safety classes an agent may use.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::Safety;

/// What an agent may do. The workspace setting "most an agent may do" caps
/// the profile an agent asks for.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    /// Reads only.
    ReadOnly,
    /// Reads, changes and moves to the trash; nothing permanent.
    Standard,
    /// Everything except operations on the application itself.
    Full,
}

impl Profile {
    /// Every profile, from least to most.
    pub const ALL: [Self; 3] = [Self::ReadOnly, Self::Standard, Self::Full];

    /// Whether an agent with this profile may run operations of `safety`.
    /// Operations of class `app` are never allowed to agents.
    #[must_use]
    pub const fn allows(self, safety: Safety) -> bool {
        match safety {
            Safety::Read => true,
            Safety::Write | Safety::Trash => !matches!(self, Self::ReadOnly),
            Safety::Permanent => matches!(self, Self::Full),
            Safety::App => false,
        }
    }

    /// The lower of this profile and `cap`.
    #[must_use]
    pub fn capped_by(self, cap: Self) -> Self {
        self.min(cap)
    }

    /// The name as written in settings and on the command line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::Standard => "standard",
            Self::Full => "full",
        }
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Profile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|p| p.as_str() == s)
            .ok_or_else(|| format!("unknown profile \"{s}\"; use read-only, standard or full"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_per_profile() {
        use Safety::*;
        let table = [
            (Profile::ReadOnly, [true, false, false, false, false]),
            (Profile::Standard, [true, true, true, false, false]),
            (Profile::Full, [true, true, true, true, false]),
        ];
        for (profile, want) in table {
            let got = [Read, Write, Trash, Permanent, App].map(|s| profile.allows(s));
            assert_eq!(got, want, "{profile}");
        }
    }

    #[test]
    fn cap_and_names() {
        assert_eq!(
            Profile::Full.capped_by(Profile::Standard),
            Profile::Standard
        );
        assert_eq!(
            Profile::ReadOnly.capped_by(Profile::Full),
            Profile::ReadOnly
        );
        for p in Profile::ALL {
            assert_eq!(p.as_str().parse::<Profile>(), Ok(p));
        }
        assert!("admin".parse::<Profile>().is_err());
    }
}
