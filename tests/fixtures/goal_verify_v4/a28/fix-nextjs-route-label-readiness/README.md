# Next.js route-label fixture

The registered acceptance boundary requires `fixture/task-02.json` to render
`ready-02` at `#result-02` on port 4185 while preserving the stable-label
regression and completing the production build.

Run the frozen checks with:

```text
node scripts/repro.mjs fixture/task-02.json
node scripts/regression.mjs
node tests/label.test.mjs
npx next build --webpack
```
