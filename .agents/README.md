# .agents

Docs for AI agents (and people) working on Carbon. Read the files that match your task; do not load everything. All project docs and research live here, nothing outside this folder.

## Start here

1. `PROJECT.md`: what Carbon is, decisions, open questions.
2. `STATUS.md`: what is done, what is next, known issues, error and solution log.

## Route by task

| If you are... | Read | Also |
|---|---|---|
| Starting a session or picking the next task | `STATUS.md`, `PROJECT.md` | `project/not_built/README.md` |
| Checking what exists in code, or changing a built feature | `project/built/README.md` | the feature's file there |
| Deciding or checking scope ("do we build X?") | `FEATURE.md` (Part 1 = Carbon features, Part 2 = scored catalog) | `PROJECT.md` pillars |
| Building or changing UI, layout, theme, text | `DESIGN.md` | `project/ui/ui-framework.md` |
| Writing GPUI / gpui-kit code | `project/ui/ui-framework.md` | `documents/gui-framework.md` (why GPUI Kit), `knowledge/rust/RUST.md` |
| Designing the daemon, protocols, data on disk, jobs | `knowledge/ARCHITECTURE.md`, `RESEARCH.md` s4-5 | `PROJECT.md` technical decisions |
| Touching security, secrets, sandbox, file access | `knowledge/SECURITY.md` | `DESIGN.md` s7 (agent interaction) |
| Hitting an error or a platform problem | `STATUS.md` error log, `knowledge/EXPERIENCE.md` | language file in `knowledge/<lang>/` |
| Asking why a decision was made, or checking evidence | `RESEARCH.md` | `documents/` |
| Comparing competitors or VS Code behaviour | `RESEARCH.md` | `FEATURE.md` Part 2 |
| Choosing or revisiting the GUI framework | `documents/gui-framework.md` | `project/ui/ui-framework.md` |
| Building docks, panels, layouts (move, snap, split, detach, saved layouts) | `project/built/panel-layout.md` | `project/ui/ui-framework.md` s5, `DESIGN.md` s4 |

## Files

| Path | Contents |
|---|---|
| `PROJECT.md` | Overview, committed pillars, technical decisions, open questions, docs map |
| `STATUS.md` | Progress checklist, next steps, decision log, error and solution log, risks, session log |
| `FEATURE.md` | Part 1 Carbon features by pillar; Part 2 feature catalog from other products with adoption scores (1-5) and unique flags; Part 3 market gaps |
| `DESIGN.md` | Carbon UI and interaction rulebook (tokens, shell, pillar surfaces, components, agent UX, accessibility, budgets) |
| `RESEARCH.md` | Landscape of alternatives, VS Code reference, comparison and synthesis, sentiment, evidence gaps, key sources |
| `documents/` | Decision documents; `gui-framework.md` compares native Rust GUI frameworks (tables) |
| `project/built/` | One file per built feature (what, code, usage, limits, verified); index in `README.md`. Panel layout comparison + implementation: `panel-layout.md` |
| `project/not_built/` | Index of committed features with no code yet (refs to `FEATURE.md` IDs); research/design docs for unbuilt features go here |
| `project/` | Working knowledge for the chosen stack; `ui-framework.md` is the GPUI Kit knowledge base |
| `knowledge/` | **Read-only** knowledge from previous projects: `DESIGN`, `ARCHITECTURE`, `SECURITY`, `EXPERIENCE`, plus per-language folders (`css`, `html`, `python`, `rust`, `shell`); see `knowledge/README.md` |
| `research_notes/` | Raw research notes behind the documents; temporary, delete once folded into the main files |

## Rules

- **Never modify `knowledge/`.** Read it, cite it, put Carbon-specific rules in `DESIGN.md`, `PROJECT.md` or `project/`.
- **Keep research inside `.agents/`.** No `research_notes/` or `reports/` at the repo root.
- **Built vs not built:** when a feature ships, add `project/built/<feature>.md` and its index row, and delete its row in `project/not_built/README.md`.
- **Update the right file:** progress, errors and decisions -> `STATUS.md`; scope -> `FEATURE.md`; settled decisions and open questions -> `PROJECT.md`; UI rules -> `DESIGN.md`; evidence -> `RESEARCH.md`; GPUI Kit facts -> `project/ui/ui-framework.md`.
- **Log errors once** in `STATUS.md` (symptom, cause, fix, prevention); reusable lessons may later move to `project/`.
- **Evidence labels:** M measured, V vendor, A anecdotal, S secondary, doc docs/spec; `(?)` = unverified. Keep them when copying facts.
- **Docs are compact:** tables and short fragments, no filler. Fold new findings into the main files instead of adding long reports.
- **`.agents/` is gitignored** and the repo has no commits yet, so deleted text cannot be recovered from git.
