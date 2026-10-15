# The Ghostty theme supplies the palette; components use scoped CSS, not DaisyUI or Tailwind

Every color in Oikonomos comes from one Ghostty theme, exposed as CSS custom properties that Svelte components style against with scoped CSS.

- The Svelte UI, CodeMirror, and the terminal renderer can only share styling through CSS custom properties, so tokens live there rather than in classes.
- One config line restyles the terminal, editor, and UI together, and any Ghostty theme works.
- In-between colors are derived from the theme's background and foreground; non-color tokens (radius, spacing, shadows, motion) are a fixed set Oikonomos owns.
- Rejected: DaisyUI, because its theme system competes with the Ghostty palette and its components don't fit the bespoke IDE layout.
- Rejected: Tailwind, because it adds a second styling approach alongside CodeMirror's themes and its utility strings get unwieldy for state-heavy components.
