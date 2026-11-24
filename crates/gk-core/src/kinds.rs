// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Small closed sets of values: priorities, colours, task-type glyphs and
//! project icons.
//! Each is stored as text (or, for priorities, a small integer) and read
//! back with `FromStr`.

use std::fmt;
use std::str::FromStr;

/// A value outside a closed set. The message lists the valid values.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("\"{value}\" is not a {what}; expected one of: {expected}")]
pub struct KindError {
    what: &'static str,
    value: String,
    expected: String,
}

impl KindError {
    pub(crate) fn new(what: &'static str, value: &str, expected: &[&str]) -> Self {
        Self {
            what,
            value: value.to_owned(),
            expected: expected.join(", "),
        }
    }

    /// The rejected text.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// What was expected, in English words (`status category`).
    #[must_use]
    pub const fn what(&self) -> &'static str {
        self.what
    }

    /// The valid values, comma-separated.
    #[must_use]
    pub fn expected(&self) -> &str {
        &self.expected
    }
}

macro_rules! text_enum {
    ($(#[$meta:meta])* $name:ident, $what:literal, { $($(#[$vmeta:meta])* $variant:ident = $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($(#[$vmeta])* $variant),+
        }

        impl $name {
            /// Every value, in display order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The stored name.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        impl FromStr for $name {
            type Err = KindError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($text => Ok(Self::$variant),)+
                    _ => Err(KindError::new($what, s, &[$($text),+])),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

text_enum!(
    /// The nine colours of the design, used for projects, statuses, types
    /// and labels. Each theme maps them to its own shades.
    Hue, "colour", {
        /// Red.
        Red = "red",
        /// Orange.
        Orange = "orange",
        /// Yellow.
        Yellow = "yellow",
        /// Green.
        Green = "green",
        /// Teal.
        Teal = "teal",
        /// Blue.
        Blue = "blue",
        /// Purple.
        Purple = "purple",
        /// Pink.
        Pink = "pink",
        /// Grey.
        Grey = "grey",
    }
);

text_enum!(
    /// The shape that shows a task's type.
    Glyph, "type glyph", {
        /// A plain task.
        Task = "task",
        /// A bug.
        Bug = "bug",
        /// A feature.
        Feature = "feature",
        /// A milestone.
        Milestone = "milestone",
    }
);

text_enum!(
    /// The icon that can stand for a project instead of its coloured
    /// circle, drawn in the project's colour. The names are those of the
    /// app's icon family (Lucide); a project without one shows the circle.
    ProjectIcon, "project icon", {
        /// Folder.
        Folder = "folder",
        /// Star.
        Star = "star",
        /// Bug.
        Bug = "bug",
        /// Rocket.
        Rocket = "rocket",
        /// Book.
        Book = "book",
        /// House.
        House = "house",
        /// Briefcase.
        Briefcase = "briefcase",
        /// Code.
        Code = "code",
        /// Terminal.
        Terminal = "terminal",
        /// Globe.
        Globe = "globe",
        /// Heart.
        Heart = "heart",
        /// Flag.
        Flag = "flag",
        /// Zap.
        Zap = "zap",
        /// Target.
        Target = "target",
        /// Lightbulb.
        Lightbulb = "lightbulb",
        /// Wrench.
        Wrench = "wrench",
        /// Hammer.
        Hammer = "hammer",
        /// Palette.
        Palette = "palette",
        /// Music.
        Music = "music",
        /// Camera.
        Camera = "camera",
        /// Graduation cap.
        GraduationCap = "graduation-cap",
        /// Shopping cart.
        ShoppingCart = "shopping-cart",
        /// Wallet.
        Wallet = "wallet",
        /// Chart line.
        ChartLine = "chart-line",
        /// Calendar.
        Calendar = "calendar",
        /// Users.
        Users = "users",
        /// Building.
        Building = "building",
        /// Plane.
        Plane = "plane",
        /// Leaf.
        Leaf = "leaf",
        /// Coffee.
        Coffee = "coffee",
        /// Gamepad.
        Gamepad = "gamepad",
        /// Puzzle.
        Puzzle = "puzzle",
        /// Shield.
        Shield = "shield",
        /// Flask conical.
        FlaskConical = "flask-conical",
        /// Layers.
        Layers = "layers",
        /// Package.
        Package = "package",
        /// Compass.
        Compass = "compass",
        /// Mountain.
        Mountain = "mountain",
        /// Sparkles.
        Sparkles = "sparkles",
        /// Database.
        Database = "database",
    }
);

/// A task's priority, stored as 0 (none) to 4 (urgent).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    /// No priority set (the default).
    #[default]
    None,
    /// Low.
    Low,
    /// Normal.
    Normal,
    /// High.
    High,
    /// Urgent.
    Urgent,
}

impl Priority {
    /// Every priority, lowest first.
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Low,
        Self::Normal,
        Self::High,
        Self::Urgent,
    ];

    /// The stored number, 0 to 4.
    #[must_use]
    pub const fn as_i64(self) -> i64 {
        self as i64
    }

    /// The priority stored as `n`.
    pub fn from_i64(n: i64) -> Result<Self, KindError> {
        usize::try_from(n)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
            .ok_or_else(|| KindError::new("priority", &n.to_string(), &["0", "1", "2", "3", "4"]))
    }

    /// The name used in commands and queries: `none`, `low`, `normal`,
    /// `high`, `urgent`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }
}

impl FromStr for Priority {
    type Err = KindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|p| p.as_str() == s)
            .ok_or_else(|| KindError::new("priority", s, &Self::ALL.map(Self::as_str)))
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip() {
        for h in Hue::ALL {
            assert_eq!(h.as_str().parse::<Hue>(), Ok(*h));
        }
        assert_eq!(Hue::ALL.len(), 9);
        for g in Glyph::ALL {
            assert_eq!(g.as_str().parse::<Glyph>(), Ok(*g));
        }
        for i in ProjectIcon::ALL {
            assert_eq!(i.as_str().parse::<ProjectIcon>(), Ok(*i));
        }
        assert_eq!(ProjectIcon::ALL.len(), 40);
        for p in Priority::ALL {
            assert_eq!(Priority::from_i64(p.as_i64()), Ok(p));
            assert_eq!(p.as_str().parse::<Priority>(), Ok(p));
        }
        assert_eq!(Priority::Urgent.as_i64(), 4);
    }

    #[test]
    fn unknown_values_list_the_choices() {
        let err = "gray".parse::<Hue>().unwrap_err().to_string();
        assert!(err.contains("grey") && err.contains("teal"), "{err}");
        assert!(Priority::from_i64(5).is_err());
        assert!(Priority::from_i64(-1).is_err());
    }
}
