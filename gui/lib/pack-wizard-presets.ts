export type PackWizardFiles = {
  assist: string;
  eval: string;
  materials: Array<{ name: string; content: string }>;
};

export function blankPackFiles(profile: string, intent: string): PackWizardFiles {
  return {
    assist: `schema_version: commandagent.pack.assist/v0
pack:
  id: local-${profile}-pack
  version: 1.0.0
  profile: ${profile}
  intent: ${intent}
inject: []
`,
    eval: "",
    materials: [],
  };
}

export const nextjsAcmeFiles: PackWizardFiles = {
  assist: `schema_version: commandagent.pack.assist/v0
pack:
  id: nextjs-acme
  version: 1.0.0
  profile: nextjs
  intent: create
inject:
  - point: project-setup
    source: pack_material_document
    required: true
    params:
      file: CONVENTIONS.md
      max_bytes: 16384
  - point: contract-wiring
    source: pack_material_document
    required: true
    params:
      file: DESIGN-RULES.md
      max_bytes: 16384
`,
  eval: `schema_version: commandagent.pack.eval/v0
pack:
  id: nextjs-acme
  version: 1.0.0
  profile: nextjs
  intent: create
checks:
  - id: path_layout_conforms
    at:
      kind: final_acceptance
    params:
      required:
        - src/app/page.tsx
        - src/app/layout.tsx
        - src/components/**
      forbidden:
        - pages/**
  - id: design_tokens_only
    at:
      kind: final_acceptance
    params:
      css_globs:
        - src/**/*.css
      tokens_file: src/app/tokens.css
      allow:
        - transparent
  - id: lint_config_present
    at:
      kind: final_acceptance
    params:
      path: eslint.config.mjs
      must_contain:
        - next/core-web-vitals
schemas:
  - artifact: package.json
    format: json
    root: object
    fields:
      - name: scripts
        type: object
        required: true
      - name: dependencies
        type: object
        required: true
    additional_fields: true
`,
  materials: [
    {
      name: "CONVENTIONS.md",
      content: `# ACME Next.js layout conventions

- Use the App Router under \`src/app/\`.
- Keep route-level composition in \`src/app/page.tsx\` and reusable UI in
  \`src/components/\`.
- Keep shared design tokens in \`src/app/tokens.css\` and import them from the
  root layout or global stylesheet.
- Do not add a parallel \`pages/\` router tree.
`,
    },
    {
      name: "DESIGN-RULES.md",
      content: `# ACME design rules

- Express colors through custom properties declared in \`tokens.css\`.
- Components consume tokens with \`var(--token-name)\` rather than raw hex,
  \`rgb()\`, or \`hsl()\` literals.
- Keep the Next.js core-web-vitals lint preset enabled.
- These rules add repository conventions; they do not replace CommandAgent's
  build, browser-interaction, or \`data-anvil-*\` hook acceptance gates.
`,
    },
  ],
};
