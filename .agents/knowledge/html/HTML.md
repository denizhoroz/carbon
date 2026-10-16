# HTML

HTML and ARIA knowledge from previous projects. General rules are in `../DESIGN.md`.


# Design

- Sort, toggle and check state through `aria-sort`, `aria-pressed`, `aria-checked`; sliders through `aria-valuetext`; live messages with `role="status"` / `role="alert"`.
- Bars and bins carry their exact count in a `data-tip` attribute.
- Decorative SVG icons: `aria-hidden="true"`, `focusable="false"`.
