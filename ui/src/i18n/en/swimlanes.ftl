# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

## Board swimlanes: bands across the columns, one for each value of a field.

# The toolbar's choice of what the lanes group the cards by.
board-lanes = Swimlanes
board-lanes-label = Swimlanes: { $by }
lanes-by-none = None
lanes-by-parent = Epic or parent
lanes-by-priority = Priority
lanes-by-label = Label
lanes-by-sprint = Sprint
lanes-by-type = Type
# The section of the select custom fields; their names are the person's own words.
lanes-by-fields = Custom fields

# The lane of the cards without a value, by what the lanes group by.
swimlane-empty = { $by ->
        [parent] No parent
        [priority] No priority
        [label] No label
        [sprint] Backlog
       *[other] No value
    }
# A parent's lane: the parent's key and title.
swimlane-parent = { $key } { $title }
# A completed sprint's lane, which can't take cards.
swimlane-completed = { $name } (completed)

# A lane as a region, for screen readers.
swimlane-label =
    { $count ->
        [one] Lane { $name }, 1 task
       *[other] Lane { $name }, { $count } tasks
    }
swimlane-collapse =
    { $count ->
        [one] Collapse lane { $name }, 1 task
       *[other] Collapse lane { $name }, { $count } tasks
    }
swimlane-expand =
    { $count ->
        [one] Expand lane { $name }, 1 task
       *[other] Expand lane { $name }, { $count } tasks
    }
# One column of a lane, for screen readers.
swimlane-cell =
    { $count ->
        [one] { $lane }, { $column }: 1 task
       *[other] { $lane }, { $column }: { $count } tasks
    }

# Why a card can't be dropped into a lane.
swimlane-refused-closed = Cards can't be moved into { $lane }: the sprint is completed.
swimlane-refused-own-parent = { $key } can't go into its own lane: a task can't be its own parent.
swimlane-refused-other-project = { $key } can't go into { $lane }: a parent must be in the task's own project.

# Moving a card with swimlanes: the lane is said with the column.
swimlane-move-done = { $key } moved to { $column } in lane { $lane }
swimlane-drag-over = { $key } is over { $column } in lane { $lane }.
swimlane-drag-end = { $key } dropped in { $column } in lane { $lane }.
swimlane-drag-instructions = To pick up a task, press Space. Use the arrow keys to move it between columns, and past the top or bottom of a column into the lane above or below. Press Space again to drop it, or Escape to cancel.
card-move-to-lane = Move to lane
