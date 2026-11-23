## The Markdown reader

reader-copy = Copy
reader-copy-code = Copy the { $language } code
reader-copied = Code copied
reader-code-plain = Text
reader-diagram = Diagram
reader-diagram-error = The diagram can't be drawn: { $error }
reader-external-image = External image: { $name }. Open in browser
reader-wiki-link = Wiki page { $page }
reader-wiki-later = The wiki page “{ $page }” will open once the wiki is part of the app.
reader-checkbox = Checklist item

## Descriptions: editing, conflict copies and history

editor-loading = Loading the editor…
description-history = History
description-source = Source
description-keys = Ctrl+Enter saves · Esc cancels
description-discard-question = Discard your changes?
description-keep-editing = Keep editing
description-discard = Discard
description-draft-kept = Unsaved changes to this description are kept from earlier.
description-draft-continue = Continue editing
description-draft-discard = Discard them
description-merged = Saved, merged with a change made meanwhile.
description-conflict-saved = Your edit clashed with another change.
description-conflict-detail = It is kept as a conflict copy; resolve it under the description.
description-conflict =
    { $count ->
        [one] An edit of this description clashed with another change and is kept as a conflict copy.
       *[other] { $count } edits of this description clashed with other changes and are kept as conflict copies.
    }
description-resolve = Resolve
description-resolve-title = Conflict copy
description-resolve-help =
    { $count ->
        [one] The lines below show how the copy differs from the description now. Keep the description, use the copy, or merge them by hand.
       *[other] The lines below show how the latest of { $count } copies differs from the description now. Keep the description, use the copy, or merge them by hand.
    }
description-copy-diff = Differences between the description and the copy
description-keep-current = Keep current
description-use-copy = Use the copy
description-merge-by-hand = Merge by hand
description-resolving = Merging a conflict copy: the text started as the copy. Saving replaces the description.
description-resolved = Conflict copy resolved
description-close = Close
description-history-help = Choose two revisions to compare them.
description-history-none = No revisions yet.
description-current = Current
description-open-copy = Conflict copy
description-revision-line = { $kind } · { $actor } · { $time }
description-kind-normal = Saved
description-kind-merged = Merged
description-kind-conflict = Conflict copy
description-kind-restored = Restored
description-kind-resolved = Resolved
description-comparing = Changes from { $older } to { $newer }
description-first = First revision, { $time }
description-restore = Restore this revision
description-diff = Differences between the two revisions
description-no-differences = No differences.
description-added-line = Added: { $text }
description-removed-line = Removed: { $text }
history-description-diff = Description changed · show the changes
comment-body = Comment
