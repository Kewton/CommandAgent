# ACME Next.js layout conventions

- Use the App Router under `src/app/`.
- Keep route-level composition in `src/app/page.tsx` and reusable UI in
  `src/components/`.
- Keep shared design tokens in `src/app/tokens.css` and import them from the
  root layout or global stylesheet.
- Do not add a parallel `pages/` router tree.
