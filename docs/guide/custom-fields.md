<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Custom fields

Custom fields give tasks values of your own, such as story points, a customer
or an approval. Add them in Project settings › **Fields**.

## Adding a field

Choose **Add a field**, give it a name and a **Kind**:

| Kind | Holds |
|---|---|
| Text | One line of text |
| Formatted text | Markdown |
| Whole number | A number such as `8` |
| Decimal number | A number such as `2.5` |
| Date | A day |
| Yes or no | Yes or no |
| One option | One option from a list |
| Several options | Any options from a list |
| Link | A web address |

Tick **For all projects** to make a field that every project has. Changes to
such a field apply everywhere.

Each field can also have:

- **Options**, for the two option kinds. Rename, add, reorder or remove them
  at any time;
- a **Default**, which new tasks get;
- **Shown for**: all task types, or only some;
- **Required**: a task needs a value. A [status rule](rules.md) can also require
  a field only when a task enters a status.

Move fields up and down to set their order in the task panel. **Delete**
removes a field with its values on every task. That can't be undone.

## Using fields

- **Task panel:** the project's fields show under **Fields**.
- **Cards:** choose them in the board's **Card fields**.
- **List:** add them as columns, group by them, and change them for many tasks
  with **Set a field…**.
- **Queries:** each field has a query name, shown as **In queries:** in the
  settings. Use it with `cf.`:

  ```text
  cf.story-points>=5
  cf.customer:"Acme"
  cf.approved:yes
  was:cf.area:"front end"
  changed:cf.story-points>-7d
  ```

- **Search:** the words of text, formatted text and link fields are found with
  the description's words.
- **History:** changes to a field are recorded like any other change.

Renaming a field keeps its history, and saved views that use it follow the new
name.

## From the command line

```sh
glyphkeel field add --project TM --name "Story points" --kind integer
glyphkeel task edit TM-41 --fields '{"Story points": 5}'
glyphkeel field list --project TM
```
