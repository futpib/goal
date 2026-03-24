# goal

[![Coverage Status](https://coveralls.io/repos/github/futpib/goal/badge.svg?branch=master)](https://coveralls.io/github/futpib/goal?branch=master)

Hierarchical goal tracker CLI.

Goals are stored in a local SQLite database and can be organised in a tree (parent → child), tagged, annotated, prioritised, and undone.

## Installation

```
cargo install --path .
```

## Usage

Running `goal` with no arguments is equivalent to `goal list`.

### Add a goal

```
goal add <description> [--parent <id>] [--continuous] [+tag …]
```

Creates a new goal and prints its ID.

- `--parent <id>` — make the new goal a child of an existing goal (ID prefix is accepted)
- `--continuous` / `-c` — mark the goal as *continuous* (ongoing, never "done"); default is *achievable*
- `+tag` — attach one or more tags to the goal at creation time

### List goals

```
goal list [+tag …]
```

Prints all pending goals as an indented tree, sorted by priority rank.
Achievable goals are shown as `[ ]`, achieved ones as `[x]`, and continuous goals as `[~]`.
Tags appear at the end of each line (e.g. `+work +urgent`).

Passing `+tag` arguments filters the tree to goals that carry all of the given tags (ancestor goals are always shown for context).

### Mark a goal done / not done

```
goal done   <id>
goal undone <id>
```

Toggles the *achieved* flag on an achievable goal. ID prefixes are accepted.

### Show goal details

```
goal info <id>
```

Prints the full details of a goal: id, kind, status, depth, parent, body, tags, annotations, and the subtree of child goals.

### Modify a goal

```
goal modify <id> [--body <description>] [--parent <id>] [--no-parent] [--continuous] [--achievable] [+tag …] [-tag …]
```

At least one option must be supplied.

| Option | Effect |
|---|---|
| `--body <text>` | Replace the goal's description |
| `--parent <id>` | Re-attach the goal under a new parent |
| `--no-parent` | Detach the goal from its current parent (make it a root goal) |
| `--continuous` / `-c` | Change the goal kind to *continuous* |
| `--achievable` | Change the goal kind to *achievable* |
| `+tag` | Add a tag |
| `-tag` | Remove a tag |

Prints the (possibly new) ID of the modified goal.

### Annotate a goal

```
goal annotate <id> <text>
goal annotate <id> <new-text> --edit   <annotation-id>
goal annotate <id>            --delete <annotation-id>
```

Adds a free-text annotation to a goal, or edits/deletes an existing one. Annotation IDs are shown by `goal info`.

### Delete a goal

```
goal delete <id> [--yes/-y]
```

Deletes a goal and all of its descendants. Without `--yes` / `-y` you are prompted for confirmation.

### Priority ranking

```
goal rank   <higher-id> <lower-id>
goal unrank <higher-id> <lower-id>
```

Sets (or removes) a priority ordering between two goals so that `goal list` displays the higher-priority goal first among siblings.

```
goal rank-interactive
```

Presents every pair of goals that have no explicit ordering yet and lets you choose which has higher priority (`1`), which is lower (`2`), skip the pair (`s`), or quit (`q`).

### Undo

```
goal undo
```

Reverses the last write operation (add, done, undone, modify, delete, annotate, rank).

### Event log

```
goal log
```

Prints a chronological history of all operations that have been performed, including timestamps, operation names, and the affected goal IDs and bodies.

## Goal IDs

Each goal is assigned a short random ID that encodes its type and depth in the tree:

- First character: `a` (achievable) or `c` (continuous)
- Second character(s): depth encoded in base-15 with `f` as the continuation digit (`0`–`e` = depths 0–14; `f` adds 15 and the next character continues, so `f0` = 15, `f1` = 16, `ff0` = 30, …)
- Remaining characters: random hex digits

Example: `a0f3c2` is an achievable root goal. `c1a9b4` is a continuous goal one level deep.

All commands that accept an ID also accept a unique **prefix** of that ID (e.g. `a0f3` instead of `a0f3c2`), as long as the prefix is unambiguous.

## Storage

The database is stored at the platform default data directory:

| Platform | Default path |
|---|---|
| Linux | `$XDG_DATA_HOME/goal/goals.db` (falls back to `~/.local/share/goal/goals.db`) |
| macOS | `~/Library/Application Support/goal/goals.db` |
| Windows | `%APPDATA%\goal\goals.db` |

Set the `GOAL_DATA_DIR` environment variable to override the data directory.
