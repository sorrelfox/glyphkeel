// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Wiki macros: `{{toc}}`, `{{children}}`, `{{include Page}}` and
//! `{{tasks <query>}}`.
//!
//! A macro stands alone on its line, as a paragraph of its own; anywhere
//! else, and in code, `{{…}}` is ordinary text. A backslash before it
//! (`\{{toc}}`) keeps it as text too. Macros are expanded only when the
//! render is given a [`MacroSource`]: wiki pages have one, descriptions
//! and comments don't.

/// The longest chain of pages included in one another: a page includes a
/// page that includes a page, and no further.
pub const MAX_INCLUDE_DEPTH: usize = 3;

/// The most pages one render includes, all levels together.
pub const MAX_INCLUDES: usize = 20;

/// One macro, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Macro {
    /// `{{toc}}`: the page's headings.
    Toc,
    /// `{{children}}`: the pages directly inside the page.
    Children,
    /// `{{include Page}}`: another page's text, by title, slug or
    /// `KEY:Page`.
    Include(String),
    /// `{{tasks <query>}}`: the tasks a query finds, as they are now.
    Tasks(String),
}

impl Macro {
    /// The macro a line holds, when the whole line (surrounding whitespace
    /// aside) is one: `{{name}}` or `{{name argument}}`, the name in any
    /// case. `{{include}}` needs a page; `{{toc}}` and `{{children}}` take
    /// nothing. Anything else is `None`.
    ///
    /// ```
    /// use gk_markdown::Macro;
    ///
    /// assert_eq!(Macro::parse(" {{toc}} "), Some(Macro::Toc));
    /// assert_eq!(Macro::parse("{{include Release plan}}"), Some(Macro::Include("Release plan".into())));
    /// assert_eq!(Macro::parse("{{tasks status:open due<7d}}"), Some(Macro::Tasks("status:open due<7d".into())));
    /// assert_eq!(Macro::parse("see {{toc}}"), None);
    /// assert_eq!(Macro::parse("{{include}}"), None);
    /// assert_eq!(Macro::parse("{{weather}}"), None);
    /// ```
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        let inner = line.trim().strip_prefix("{{")?.strip_suffix("}}")?;
        if inner.contains("{{") || inner.contains("}}") {
            return None;
        }
        let inner = inner.trim();
        let (name, argument) = match inner.split_once(char::is_whitespace) {
            Some((name, rest)) => (name, rest.trim()),
            None => (inner, ""),
        };
        match name.to_ascii_lowercase().as_str() {
            "toc" if argument.is_empty() => Some(Self::Toc),
            "children" if argument.is_empty() => Some(Self::Children),
            "include" if !argument.is_empty() => Some(Self::Include(argument.to_owned())),
            "tasks" => Some(Self::Tasks(argument.to_owned())),
            _ => None,
        }
    }

    /// Its name, as the HTML marks its output (`data-macro`).
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Toc => "toc",
            Self::Children => "children",
            Self::Include(_) => "include",
            Self::Tasks(_) => "tasks",
        }
    }
}

/// The macros of `source`, in reading order, each with the line it is on
/// (from 1).
///
/// ```
/// let found = gk_markdown::macros("# Plan\n\n{{toc}}\n\n`{{toc}}`\n\n{{tasks label:release}}\n");
/// assert_eq!(found.len(), 2);
/// assert_eq!(found[1].0, 7);
/// ```
#[must_use]
pub fn macros(source: &str) -> Vec<(usize, Macro)> {
    use comrak::nodes::NodeValue;
    use comrak::{Arena, parse_document};
    let arena = Arena::new();
    let options = crate::options();
    let root = parse_document(&arena, source, &options);
    let lines = Lines::new(source);
    root.descendants()
        .filter(|n| matches!(n.data().value, NodeValue::Paragraph))
        .filter_map(|n| {
            let pos = n.data().sourcepos;
            lines.paragraph_macro(pos).map(|m| (pos.start.line, m))
        })
        .collect()
}

/// A page a macro lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageItem {
    /// What a wiki link to it names: its title, or `KEY:Title` for a page
    /// of another project.
    pub target: String,
    /// Its title.
    pub title: String,
}

/// A page to include.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Included {
    /// Its ID, so that a page isn't included inside itself.
    pub id: String,
    /// Its text now.
    pub text: String,
}

/// A task a `{{tasks}}` macro lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskItem {
    /// Its key, such as `TM-41`.
    pub key: String,
    /// Its title.
    pub title: String,
    /// Its status's name.
    pub status: String,
    /// Whether that status counts as done.
    pub done: bool,
}

/// What a `{{tasks}}` macro finds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskList {
    /// The first tasks, in the query's order.
    pub items: Vec<TaskItem>,
    /// How many tasks match in all.
    pub total: usize,
}

/// Where macros get what they show. `page` is the page whose text holds
/// the macro: `None` for the text being rendered, otherwise an
/// [`Included::id`].
pub trait MacroSource {
    /// The ID of the page being rendered, if it is one, so that it isn't
    /// included in itself.
    fn page(&self) -> Option<&str>;
    /// The pages directly inside `page`, in order.
    fn children(&self, page: Option<&str>) -> Vec<PageItem>;
    /// The page `name` names, seen from `page`; `None` when there is none
    /// the reader may see.
    fn include(&self, page: Option<&str>, name: &str) -> Option<Included>;
    /// The tasks `query` finds, seen from `page`; the error says what is
    /// wrong with the query.
    ///
    /// # Errors
    ///
    /// The query's mistake, as text for the reader.
    fn tasks(&self, page: Option<&str>, query: &str) -> Result<TaskList, String>;
}

/// A line the reader shows in place of, or after, a macro's output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// `{{toc}}` on a page without headings.
    NoHeadings,
    /// `{{children}}` on a page without subpages.
    NoChildren,
    /// `{{include}}` of a page that doesn't exist (or the reader may not
    /// see).
    IncludeMissing(String),
    /// `{{include}}` of a page that is already being included around it.
    IncludeLoop(String),
    /// `{{include}}` nested deeper than [`MAX_INCLUDE_DEPTH`].
    IncludeTooDeep(String),
    /// More than [`MAX_INCLUDES`] includes in one render.
    IncludeTooMany(String),
    /// `{{tasks}}` that finds nothing.
    NoTasks,
    /// `{{tasks}}` that finds more than it lists: how many more.
    MoreTasks(usize),
    /// `{{tasks}}` with a query that can't run: why.
    TasksError(String),
}

impl Note {
    /// Its kind, as the HTML names it (`data-macro-note`).
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::NoHeadings => "toc-empty",
            Self::NoChildren => "children-empty",
            Self::IncludeMissing(_) => "include-missing",
            Self::IncludeLoop(_) => "include-loop",
            Self::IncludeTooDeep(_) => "include-too-deep",
            Self::IncludeTooMany(_) => "include-too-many",
            Self::NoTasks => "tasks-empty",
            Self::MoreTasks(_) => "tasks-more",
            Self::TasksError(_) => "tasks-error",
        }
    }

    /// Its value, as the HTML carries it (`data-macro-arg`): a page's name,
    /// a count or an error; empty when it has none.
    #[must_use]
    pub fn value(&self) -> String {
        match self {
            Self::NoHeadings | Self::NoChildren | Self::NoTasks => String::new(),
            Self::IncludeMissing(page)
            | Self::IncludeLoop(page)
            | Self::IncludeTooDeep(page)
            | Self::IncludeTooMany(page) => page.clone(),
            Self::MoreTasks(count) => count.to_string(),
            Self::TasksError(error) => error.clone(),
        }
    }
}

/// The source's lines, to read a paragraph as written.
pub(crate) struct Lines<'s> {
    lines: Vec<&'s str>,
}

impl<'s> Lines<'s> {
    pub(crate) fn new(source: &'s str) -> Self {
        let mut lines = Vec::new();
        let mut rest = source;
        while !rest.is_empty() {
            let end = rest.find(['\r', '\n']).unwrap_or(rest.len());
            lines.push(&rest[..end]);
            rest = &rest[end..];
            if rest.starts_with("\r\n") {
                rest = &rest[2..];
            } else if !rest.is_empty() {
                rest = &rest[1..];
            }
        }
        Self { lines }
    }

    /// The macro a paragraph at `pos` is, when it is one line holding only
    /// a macro.
    pub(crate) fn paragraph_macro(&self, pos: comrak::nodes::Sourcepos) -> Option<Macro> {
        if pos.start.line != pos.end.line || pos.start.line == 0 {
            return None;
        }
        let line = self.lines.get(pos.start.line - 1)?;
        let text = line.get(pos.start.column.checked_sub(1)?..pos.end.column.min(line.len()))?;
        Macro::parse(text)
    }
}
