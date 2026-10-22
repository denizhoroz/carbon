# Status bar

Built 2026-10-16. User request: compact bottom bar; left to right: workspace name, source branch, jobs. First GUI feature reading `carbon-core`.

## What

| Item (left to right) | Icon | Shows | Source |
|---|---|---|---|
| Workspace | `folder` | Folder name, or "No folder"; it is a button: menu (opens upward) with Open Folder... / Create Folder... / Close Folder (`workspace-folders.md`) | `Core::workspace()` |
| Branch | `git-branch` | Branch name; short commit id when detached; hidden outside a git repo | `Core::branch()` reads `.git/HEAD` (worktrees and subfolders supported), re-read every 2 s |
| Jobs | `list-checks` | "No jobs", the job title when one runs, or "N jobs" | `Core::jobs.running()` |

Bar: kit `StatusBar` (theme `status_bar` colours, top border, `text_xs`), height = `fixed::BAR_HEIGHT` (30 px since 2026-10-17: same as the title/menu bar and the tab bar; user request), full window width under the workbench; at the right end, after the language servers, the **app menu** button (Lucide `menu`, three lines; 2026-10-22, user): an upward dropdown with Extensions, Settings (the placeholder Manage dialogs, `manage.rs`), separator, Command Palette. Click-tested with real mouse input; 12 px side padding, 14 px icons, 4 px icon-text gap, 8 px between items.

Opening a folder: File menu, this button, the Explorer, or `carbon-gui <folder>` (`workspace-folders.md`).

## Code

| File | Role |
|---|---|
| `apps/carbon-gui/src/status_bar.rs` | `CarbonStatusBar` view; observes the `Core` entity; branch poll task |
| `apps/carbon-gui/src/main.rs` | Creates `Entity<Core>`, opens the folder argument, places the bar |
| `apps/carbon-core/src/vcs.rs` | `current_branch`, `Branch` |
| `apps/carbon-core/src/jobs.rs` | `JobStore`, `Job`, `JobStatus` |
| `apps/carbon-gui/src/app_assets.rs` | Adds `GitBranch`, `ListChecks` icons |

## Verified

- Screenshot with this repo as workspace: "carbon", "main", "No jobs".
- Core tests `tests/status.rs`: branch named/detached/subfolder/worktree, jobs lifecycle (5 tests).

## Limits

- Branch is polled (2 s), not watched; nothing else reads git status (dirty count, ahead/behind) yet.
- Nothing starts jobs yet, so it always says "No jobs"; no spinner or Stop (`DESIGN.md` s4 wants both).
- Branch and jobs items are not clickable (no branch picker, no jobs list).
- DESIGN s4 also wants dock toggles, diagnostics, agents and cost in this bar; not built.
