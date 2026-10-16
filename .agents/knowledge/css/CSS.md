# CSS

CSS knowledge from previous projects. General rules are in `../DESIGN.md`.


# Design

- Size, spacing and motion tokens at the top of the main stylesheet (`--space-*`, `--text-*`, `--duration`, `--ease-out`); colour and font tokens in a separate themes stylesheet with the token contract written at its top (`--color-fg`, `--color-fg-muted`, `--color-accent-on`, `--viz-*`, …).
- Themes switch by an attribute on the root element; the text scale by a `--font-scale` variable set from a `data-scale` attribute. The `--text-*` tokens are built from it.
- Never `display: flex` on `td` / `th`; put a flex `span` inside.
- Bar lists use `subgrid` so rows share columns.
- An entity colour reaches its shapes as a custom property (e.g. `--shape-color`), not one class per colour.
- `@media (prefers-reduced-motion)` for motion; remove toasts on `animationend`, never `setTimeout`.
- `:focus-visible` for focus rings.
