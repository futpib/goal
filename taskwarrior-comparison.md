# Taskwarrior vs `goal` — CLI comparison

## Add

| | Taskwarrior | `goal` |
|---|---|---|
| Basic | `task add "description"` | `goal add "description"` |
| With metadata | `task add "desc" due:Friday priority:H project:Work +tag` | `goal add "desc" --continuous` |
| Parent/hierarchy | `task 2 modify depends:1` (after the fact) | `goal add "desc" --parent <id>` (at creation) |

Taskwarrior attaches hierarchy via `depends` as a modification; `goal` sets parent at creation time.

## Done / Complete

| | Taskwarrior | `goal` |
|---|---|---|
| Mark done | `task 1 done` | `goal done <id>` |
| Undo done | `task undo` (global undo) | `goal undone <id>` (targeted) |
| Mark not done | no direct command | `goal undone <id>` |

Taskwarrior's `undo` is a global last-action undo. `goal undone` directly targets a specific goal.

## Delete / Remove

| | Taskwarrior | `goal` |
|---|---|---|
| Remove | `task 1 delete` (prompts confirmation) | `goal delete <id>` (no prompt) |

## List / View

| | Taskwarrior | `goal` |
|---|---|---|
| Default list | `task list` / `task next` | `goal list` |
| Filtered | `task +tag project:X list` | — |
| Single item | `task 1 info` | — |

## Notable gaps in `goal`

- No `modify` — can't edit body or reparent after creation
- No filtering/querying
- No way to view a single goal's detail or subtree
- No `undo`
