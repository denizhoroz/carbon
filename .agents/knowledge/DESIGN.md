# Design Knowledge

UI design choices from previous projects, kept to reuse in future ones. No language or framework details here; those live in the language folders (see `README.md`).

Related: `ARCHITECTURE.md` (system design), `SECURITY.md` (safety and privacy), `EXPERIENCE.md` (problems met and how they were solved).


# 1. Principles

- **One rulebook.** Keep design rules in one file and read it before any UI build or fix. A rule asked for once ("never do X again") goes there, not in feature specs or change logs, so it holds everywhere, for old and new screens.
- **Consistency over local cleverness.** Same meaning, same look: one table pattern, one output picker, one format list, one colour per entity, one icon per meaning. A local copy (a palette, a list of formats) always drifts; feed every screen from one shared source.
- **Only offer what applies.** Hide options that can't apply to the current input, and the result fields about them. Show an option disabled, with the reason, only when the user can change something to make it apply. Exception: a list that must look the same everywhere (e.g. file formats) shows the unsupported entries disabled with "(not supported yet)".
- **Say it once.** Don't repeat a number in a tile when a list below already shows it; no version line next to a status that already says enough.
- **Don't warn about yourself.** Never warn about a name, value or state the app itself just chose.
- **Plain words.** Short, plain sentences that say what happens ("Files are copied into the project folder").


# 2. Foundations (tokens)

- **Everything is a token.** Spacing, type sizes, radii, motion, layout widths, every colour and font are named tokens defined in one place. Rules use tokens only; no raw colours, no raw sizes when a token fits.
- **Spacing scale** on a 4 px base with a 2 px half step: 2, 4, 8, 12, 16, 20, 24, 32.
- **Type scale**: a few steps (xs, sm, base, xl, 2xl), multiplied by the text size the user picks in Settings. Never a fixed font size that ignores the user's scale.
- **Fonts**: a monospace font first (JetBrains Mono), plus bundled readable alternatives (Inter, IBM Plex Sans, Atkinson Hyperlegible Next) the user can pick. Bundle everything: fonts, icons, styles. A local app loads nothing from the internet.
- **Themes**: built purely from colour tokens. Many themes, half dark and half light (16 worked well: own dark + light pair, plus Catppuccin Mocha / Latte, Rosé Pine / Dawn, Tokyo Night / Day, Gruvbox Dark / Light, Dracula, Nord / Nord Light, Everforest Dark / Light, Solarized Light), one dark default. A theme fills every token in a written contract; a new token means every theme gets a value. Themed thin scrollbars.
- **Contrast contract**, checked for every theme: body and muted text >= 4.5:1 on background and surface; text on the accent >= 4.5:1; chart colours >= 3:1 on surface.
- **Icons**: one inline line-icon set (Lucide style) from one icon component; new icons are added there, never as loose image files.
- **Motion**: short durations, ease-out, respects the OS "reduce motion" setting. Animations end on their own end event, not on timers (toasts leave when their animation ends).


# 3. Layout

- **App shell**: sidebar (collapsible feature groups, then History and Settings) | page | resizable side panel. Status bar along the bottom. Below 768 px the sidebar is a drawer and nothing scrolls sideways.
- **Pages**: fixed max width (960 px normal, 1200 px wide) with an eyebrow (the sidebar group), a title and a one-sentence lead.
- **Panels / cards**: bordered surface, 16 px padding, 16 px gap inside. They stretch with the content column (narrow and widen with the side panel), never a fixed width.
- **Nothing sticks together.** 20 px between stacked cards; the same gap above any card, notice, progress or result that follows a form or button row. The generic "gap above a panel" rule must not hit grid children: grid cells in one row must be the same height.
- **Grids of cards**: no holes, cards in one row equal height, content aligned to the top.
- **Titles live inside their card's border**, including form-group legends.
- **Tabs inside a page**: a segmented control (e.g. Create | Edit | Download | Import) with a gap below.
- **Side panel**:
  - Resizable with a minimum width (360 px); never makes the window scroll: the layout and the panel clip, the table scrolls in the panel body, the selection bar wraps, popups stay inside the panel.
  - Holds switchable tables (one per main kind of record), switched from its title.
  - Tools take their input from the side panel's selection; no pickers for the same thing on tool pages.
  - A page that works on one kind of row opens the panel on that table itself and shows a hint ("Select an item in the items panel" + arrow) until one is chosen.
  - A page that needs a pick from another table switches the panel with a "Choose in the … table" button.
  - A page that only reads the selection uses the last selected item at once and follows selection changes.
- **Empty states are exact.** Nothing selected: one hint line with an arrow toward the panel, nothing else. No project open: Create and Load buttons.


# 4. Components and patterns

## Tables (every table, old and new)

- Checkbox column (17 px checkboxes); a row click selects only that row (again: clears); a name button in the row for keyboard users.
- Selection bar: Select all / None / red **Remove** / "N selected".
- `#` id column; sortable headers (text A–Z first, counts largest first, ids lowest first); a count and a reload button in the header.
- The selection is a list in selection order; single-item pages use the last one.
- Deleting is Remove in the table. No separate Delete sections or pages.
- Remove asks "Delete N …? … can't be undone" in a small popup under the button, never a box that pushes the table down. Backdrop click and Escape cancel.
- **Fixed lists** (e.g. a set of options to include): the checkbox means "included", with Select all / None and the count, no Remove; the reload icon resets to the defaults.
- **Result tables**: sortable with `#` and a count, no checkboxes: nothing to act on.
- **Rows with many settings**: the settings go on a line under their row (spanning the columns after `#`), so the table never scrolls sideways. Row and settings line share one band: a strong line between bands, none inside, a soft accent background when used. Each setting is its own small boxed group (label + boxes). A column of controls (sliders) is left-aligned with its header, not right-aligned like numbers.
- **Table cells stay table cells**: a flex wrapper goes inside the cell, the cell itself never becomes flex (its row border breaks).
- **Special rows get a badge**: e.g. a bordered "Link" badge (accent border and text) right after a linked item's name; the column about it ("Linked to") sits right after Name and only appears while some row needs it.
- A search box above the table where lists get long.

## Explanations, warnings, feedback

- **Explanations are hover icons**: an "i" button (shows on hover, stays on focus/click) next to the label or title, not a line of small text. Small text stays only for live values ("Saved as …", name examples, "Using seed N"), errors, and safety notes that must be seen ("Replaces X" on an overwrite option).
- **Warnings are icons**: many warnings never fill the page; a warning icon in the card header lists them on hover/focus.
- **Popups escape their containers**: tips and popups inside tables or scroll boxes are placed against the window, never clipped or covered by what comes after them.
- **Toasts** for finished actions. The status bar shows running jobs and loads with a spinner and Stop; a small animated "!" above the spinner when a job starts, no popup.
- **Status pills and tags**: rounded border with a matching soft background, and always a word. Done / installed = green (accent), Failed / not installed = red, Warning = yellow, Skipped = grey (e.g. "already exists").

## Inputs and forms

- **Numbers with a sensible range are sliders**: the range plus the value shown next to it, optional text for a special value (0 = "Automatic"). A fixed value width so "Automatic (N)" doesn't move the track. Free-range numbers (seeds, sizes up to thousands, tuning values) stay number boxes.
- **Form grids**: one field per column; number boxes and dropdowns are the column's width. Checkbox rows use the same columns so they line up with the fields above; checkboxes are centred on their text. An option that modifies an input (e.g. "Auto") sits to the right of the box.
- **Picking several names**: toggle chips. None picked means all, and then every chip shows as picked so it never looks like nothing is chosen; clicking one keeps only it; an "All" button clears the choice.
- **Choices**: radio cards; the whole card is clickable and holds its pill and description; its action buttons sit below, outside the card.
- **Form actions**: primary button first, then ghost buttons; notes beside them ("Saved.", "Resolve the problems in Result first.") vertically centred.
- **Side-by-side columns with an action each**: the buttons line up at the bottom whatever sits above them; the chosen-item boxes fill their columns at the same height.
- **Optional sections** that help fill a form are open by default. **Logs and consoles** are collapsible and closed by default; the progress panel carries the status.
- **Picking files**: one form takes a folder or a zip, told apart by the path (".zip"), with a "Choose folder" and a "Choose zip" button (the system dialogs pick one kind each). Don't ask which kind first.
- **Tool output**: one picker everywhere: "Save as new" (default) / "Overwrite source" with an "I understand … can't be undone" checkbox. The new name is a full name prefilled as `<source>-<suffix>` (e.g. `-fixed`); with several inputs, one name field per input. Never ask for a suffix alone.
- **Suggested names** are short, made of what the thing is built from (`<input>-<tool>`), numbered when taken.
- **Colours** of an entity are changed in one place only, with a colour picker: a swatch button opening 16 presets (from a 64-colour palette) in a 4×4 grid, current one outlined, then "Custom colour…" with the system picker. Escape or an outside click closes. Never a bare system colour input.

## Content display

- **Pictures**: any set of images is a gallery grid of tiles with captions; a click opens an image viewer (dialog, ←/→ to step, Esc to close). No bare thumbnails or links to raw files.
- **Entity colours** (e.g. categories or tags): one colour per entity, the same everywhere it appears, stored with the data, never a local palette. Text on these colours is dark (#111). Charts keep their single data hue. Small swatches (12 px) next to mapping choices; dashed outline when dropped / unmapped.
- **Title icons**: every card, settings section and table title starts with a line icon in the accent colour. Same meaning, same icon: Result → chart, Output → save, Inputs → database, Options → sliders, categories → tag, images → image. Image captions get none. Users can hide them all.
- **Big headline numbers**: the main figure (e.g. a health percentage) is the big text; its supporting numbers sit under it in the same box, not in separate tiles.

## Settings

- Sections by concern: Appearance, System, Cache, devices / runtimes, API keys.
- A choice between modes says what each means in one line ("Runs on the GPU. Faster than CPU."); no version line next to the status.
- API keys: "Saved" only, never part of the key; the line under a field is only the "Where to find <service> API token" link; what the key is for goes in the label's info tip.
- System: limits sized to the machine (jobs at once up to what its cores and memory allow, shown in an info tip).
- The open project's own options are not in Settings: a gear at the top right of the current-project card opens them.

## Jobs and history

- **A page shows the job it is running or just ran.** After switching sections and back, finished results are gone from the page and live in the History overlay.
- **History overlay** beside its sidebar button: every job and one-step action as category > section > process, with start / end time, items, outcome and details. Esc or click outside closes. Stored in the project cache and cleared with it.
- **Jobs**: one per tool at a time; refused when a running job uses the same data or the "jobs at once" limit is reached; every job can be cancelled; per-item outcome (done / failed / warning / skipped / cancelled), re-run of a failed item.

## Canvas editors

- Own route with the item index in the URL, so reload / back keep the place. Leaving (sidebar link, Close, Esc with nothing selected) saves first.
- Layout: sidebar | tools strip | canvas with top bar | resize handle | side list (replaces the side panel's tables, shares its width).
- Top bar: item name, state pills, save state; `current / total` (type a number + Enter to jump); filter (All / Not done), Previous, Next, Close.
- Tools strip: icon buttons with tooltip + single-key shortcut, one active tool: Select / move (V; drag moves, 8 handles resize, Shift+click adds), Draw (B; stays active after a draw), Pan (H or hold Space), Delete (Del / Backspace). Below: Undo (Ctrl+Z), Redo (Ctrl+Y / Ctrl+Shift+Z), Zoom (+ / −, wheel), Fit (0), overlay visibility (L).
- Canvas: overlay in the content's own coordinates so shapes match what is stored; zoom / pan via transform. Shapes clamped to the content; shapes under 2 px dropped. Arrow keys nudge 1 px (Shift: 10 px); copy / paste shapes, also onto the next item; A / D or ← / → step when nothing is selected. Undo history per item, cleared on item change. The next item preloads.
- Side list: a full table; `#` starts at 1 so keys 1–9 pick the first nine rows; search above; last row "+ Add"; rename by double-click or F2; Remove confirms with the counts it will delete.
- Autosave: on item change, on leave, on Ctrl+S, and 1.5 s after the last change. Save state always visible ("Saved" / "Saving…" / "Save failed · Retry · Discard"); navigation blocked until saved or discarded.
- Refused (message + back link) while a job touches the data; read-only banner if one starts while open.


# 5. Charts

- Chart colours come from dedicated chart tokens only (series, track, ramp, overlay). Building blocks: bar list, histogram, heatmap, line chart, stat tile.
- **Line charts**: small multiples, one quantity per chart, rather than many colours in one. A pair of related series shares the hue: first solid, second dashed, named in a legend. Y range from the data with its min and max labelled; first and last x labelled; a hover column per x with every value; a dotted marker for a highlighted x (e.g. the best point); a dotted level line for a reference value, named with its value in the legend and the hover text.
- **Bar lists**: rows share the list's columns, so a longer value never shortens its bar; values right-aligned in tabular numbers.
- Every chart has an accessible name; bars and bins have a tooltip with the exact count. A clickable bar is a button (e.g. filters a list).
- Shapes drawn over content: each entity in its own colour, the overlay tokens when it has none.
- Numbers through one formatter (whole numbers without decimals).


# 6. Text

- Sentence case for labels and titles. Buttons say the action ("Add item", "Convert", "Remove"); while working: "Adding…", "Working…".
- Destructive confirmations name the count and say it can't be undone.
- Counts and plurals through shared helpers, never written by hand.


# 7. Accessibility

- Every control has a visible label or an accessible name; tips, warnings and sliders carry their text for screen readers (label, value text).
- Visible focus; full keyboard use (tables, split handles, sliders, editor shortcuts); live regions for status and alerts; sort / pressed / checked states exposed.
- Text contrast >= 4.5:1 in every theme; never colour alone: pills and levels also say it in words.


# Before finishing a UI change

- Build every target; look at the page running when possible.
- Check spacing, alignment and equal heights against these rules in a dark and a light theme and at a large text size.
- Log the change; if it adds or changes a rule, update the rulebook.
