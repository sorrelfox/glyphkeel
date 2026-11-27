# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# Today: its sections and rows; and dates typed in words in the task panel.

## Sections

today-section-overdue = Overdue
today-section-due-today = Due today
today-section-in-progress = In progress
today-section-reminders = Reminders
today-section-claimed = Agents at work
today-section-recent = Changed in the last day
today-section-timer = Timer
today-more = { $count ->
    [one] { $count } more task is not shown.
   *[other] { $count } more tasks are not shown.
  }

## Rows

today-due-today = Today
today-reminder-note = { $time }: { $note }
today-claimed-by = { $agent }, until { $time }
today-changed-by = { $who }, { $time }
today-me = Me
today-timer-running = Running, { $elapsed }
today-timer-paused = Paused, { $elapsed }

## Dates typed in words (task panel)

date-words-label = { $name } in words
date-words-placeholder = next fri, in 3d, dec 1
day-picker-hint = Type a day in words, or pick one.
# The words typed in a date field don't name a day.
day-picker-unknown = Not a date. Try next fri, +3d or 2026-12-01.
day-picker-previous-month = Previous month
day-picker-next-month = Next month
day-picker-today = Today
day-picker-clear = Clear
