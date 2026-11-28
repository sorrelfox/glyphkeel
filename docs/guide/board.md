<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The board

The board shows a project's tasks as cards in columns, one column for each
status. Open it with **Board** in the header bar, or Ctrl+1.

<img src="../screenshots/board.png" alt="The Glyphkeel board in the light theme: a sidebar with Today, All tasks and three projects, a filter bar, and five columns from Backlog to Done with task cards showing labels, due dates, checklist progress, comments and time spent" width="900">

## Moving cards

- **Drag** a card to another column, or to another place in its column.
- **With the keyboard:** select a card and press Space to pick it up. The arrow
  keys move it between and within columns. Space drops it, and Escape cancels.
- **Shortcuts:** Alt and an arrow key move the selected card to the column on
  the left or right, or up and down in its column.
- **Card menu:** right-click a card, or press the menu key, for **Open**,
  **Move to**, **Priority**, **Copy key** and **Move to trash**.

Enter opens the selected card, and the arrow keys move between cards.

A move that a [status rule](rules.md) doesn't allow is refused, with the reason:
for example a field that must be filled in first.

## Your own layout

How you arrange the board is your own layout: it changes nothing in the
project's set-up. In each column's menu you can:

- **Hide** the column. The bar above the board says how many columns and tasks
  are hidden; click it to show a column again;
- **Collapse** it to a narrow strip;
- **Move left** and **Move right**. Alt+Shift and an arrow key move the column
  of the selected card;
- **Sort** it by **Manual order**, **Priority**, **Due date** or **Last
  updated**. In a sorted column, cards can't be moved by hand.

**Reset to project layout** goes back to the project's order.

The board options button sets the **Card fields** (Labels, Due date,
Checklist, Subtasks, Start date, Estimate, Days in column, and custom fields),
the card size (**Regular** or **Compact**) and **Hide closed cards after** a
number of days. A column with hidden closed cards has **Show older**.

## Card colours and covers

**Card colour** in the board options gives each card a coloured edge:

- **By type**, **By priority** (urgent red, high orange, normal blue, low
  grey) or **By label** (the card's first label);
- **By rules**: **Edit colour rules…** lists queries in the
  [query language](../query-language.md), each with a colour and an optional
  name. A card takes the colour of the first rule it matches, so put the most
  important rule first. A rule that no longer reads, say after its label was
  deleted, colours nothing.

The colour is never the only sign: hovering the edge names the rule
(**Colour: priority Urgent**), and screen readers read it with the card.

**Covers** shows an image at the top of each card. In the task's **Files**,
an image's menu has **Use as card cover**. **Chosen images** shows only the
images chosen that way; **Chosen or first image** also shows the first image
of a task without one. A cover is the image's thumbnail, loaded when the card
comes into view.

Colours and covers are part of your layout, so Undo takes a change back. From
the command line: `glyphkeel board layout set TM --colour-by priority`,
`glyphkeel file cover TM-41 --attachment <id>`; agents use the
`set_board_layout` and `set_card_cover` tools.

`glyphkeel board TM` prints the board column by column: each column's name,
count and limits, then its cards, one a line, with the key, the title, the
name of what colours the edge (`edge: Urgent bugs`) and the cover's file name
(`cover: shot.png`). With `--json` it prints the whole board.

## Swimlanes

**Swimlanes** in the board's toolbar adds horizontal lanes across the
columns, so one board can be read by **Epic or parent**, **Priority**,
**Label**, **Sprint**, **Type** or a select custom field. **None** goes back
to plain columns.

- Each lane has a header with its name and its number of tasks, and each of
  its columns shows how many of the lane's cards it holds. The column headers
  at the top keep the column's total.
- Cards without a value go in the last lane (**No priority**, **No label**,
  **No parent**, **Backlog** for sprints). Priorities, types, open sprints
  and a field's options always have their lane, even while it is empty;
  labels and parents have one while a card shows them. A card with several
  labels is in the lane of its first one.
- The arrow in a lane's header collapses it. Which lanes are collapsed is your
  own setting, kept on this computer for each project and grouping, so they
  stay collapsed when the board opens again; it isn't part of the layout and
  isn't in the history.
- Dragging a card into another lane changes the lane's field as well, and
  into another column the status too, in one change that one Undo takes back.
  A lane that can't take the card says why while you drag: a completed
  sprint, a task's own lane as its parent, or a parent in another project.
- From the keyboard, a card's menu has **Move to** (another column, same
  lane) and **Move to lane** (another lane, same column). While a card is
  picked up with Space, the arrow keys move it between columns, and past the
  top or bottom of a column into the lane above or below. Screen readers hear
  the lane and the column of each step.
- WIP limits stay the column's: the column header warns on the column's
  total, whatever the lanes.

The choice is part of your layout, so Undo takes it back too. From the
command line, `glyphkeel board TM --lanes priority` prints the column totals
and then each lane with its columns and cards; `glyphkeel board layout set TM
--lanes field --lane-field cf.area` keeps a grouping. Agents read the lanes with `get_board`
(each lane with its counts by column, each card naming its lane), set them
with `set_board_layout`, and move a card into a lane with `move_task` and its
`changes`.

## Filtering

The filter bar above the board narrows the cards:

- **Filter by title or key**;
- **Label**, **Priority** and **Due**: Overdue, Due today, Due in the next 7
  days or No due date;
- **Recent**: recently updated tasks.

The board says how many tasks the filter shows. **Clear filter** shows them all.
**Include sub-projects** adds the sub-projects' tasks.

For more complex filters, use the [list](list.md) and the
[query language](../query-language.md).

## WIP limits

A status can have a **WIP limit** and a **WIP minimum** (in Project settings ›
Statuses). The column header shows the count against the limit, such as `4 / 5`,
and marks a column that is over its limit or under its minimum. The limits are
shown as a warning; they don't stop a move.

## Cards

A card shows its key, title, type and priority, and the fields you chose. It
also shows comments, files, logged time, a **Blocked** mark when an open task
blocks it, and the agent that has claimed it.
