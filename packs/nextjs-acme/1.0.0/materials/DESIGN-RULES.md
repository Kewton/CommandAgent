# ACME design rules

- Express colors through custom properties declared in `tokens.css`.
- Components consume tokens with `var(--token-name)` rather than raw hex,
  `rgb()`, or `hsl()` literals.
- Keep the Next.js core-web-vitals lint preset enabled.
- These rules add repository conventions; they do not replace CommandAgent's
  build, browser-interaction, or `data-anvil-*` hook acceptance gates.
