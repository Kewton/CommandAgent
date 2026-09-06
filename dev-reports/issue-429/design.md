# Issue #429: register lazy JSON outputs before Recovery observation

The previous Next.js policy enumerated existing JSON files and matched their
names against source containing writer-like text. The replacement recognizes a
small static language in route-bound source, before any output exists. It does
not execute source to discover permissions and does not grant a directory.

## Recognition and authority

- Registration is limited to an App Router project at the workspace root.
  Nested layouts and registered commands with cwd selectors (`cd`, `--prefix`,
  `-C`, `--cwd`, workspace flags, etc.) grant no paths. Route-bound `chdir`
  references also disable grants conservatively. A source file's directory is
  never used as an execution-root guess. Nested support would need a separately
  product-bound execution root; the root-layout S2 writer remains supported.
  The same cwd checks apply to all root `package.json` script bodies, covering
  dev/build/start, lifecycle hooks, and indirectly invoked scripts. An unrelated
  cwd-changing script can conservatively suppress grants; ordinary
  `next build`/`next dev`/`next start` scripts remain supported.
- Start from App Router page, layout, and route entrypoints in the existing
  route closure. Recheck import edges using tokens, so quoted or commented
  imports cannot introduce an unregistered sibling. Follow relative and `@/`
  imports, including literal dynamic imports, within that closure.
- Recognize builtin `fs`, `fs/promises`, and `path` imports (also `node:` forms),
  default/namespace imports and supported named imports with aliases.
- Recognize `writeFile` / `writeFileSync` calls bound to those imports, literal
  relative paths, `process.cwd()`, `path.join`, and immutable simple `const`
  initializers ending in a semicolon. Resolve constants in their lexical brace
  scope, including separate local `filePath` declarations in different writers.
- Require the entire first writer argument, and each complete initializer, to
  match. A static prefix followed by concatenation, a conditional, or another
  dynamic expression grants nothing. Deduplicate normalized exact file paths.
- Required and protected paths, source directories, JSON configuration files,
  hidden/internal namespaces, absolute paths, traversal, and symlink components
  are ineligible. Existing output files are optional; existing directories are
  not output files. Snapshot hashing excludes exact registered files, never
  descendants under a directory named like an allowed file.
- Configuration exclusions include `tsconfig.*.json` / `jsconfig.*.json`, JSON
  imported as source, and transitive JSON references from known/contract-bound
  configuration files (including extensionless `extends`). Reference matches
  only remove authority; conservative matches in comments/quoted import text
  are safe false negatives. JSON imports are protected even when their source
  contains syntax excluded from writer recognition.
  Config sources such as `next.config.js`/`.ts` are scanned independently of
  route reachability. Conservative quoted-reference extraction supplements JSON
  parsing for JSONC comments/trailing commas; parse failure cannot drop these
  exclusions. These exclusions do not add supported writer syntax.

## Conservative limits

This is not a general JavaScript/TypeScript parser. Regex or division tokens and
template literals cause the source file to grant no outputs. Comments are
discarded and quoted literals are atomic; escaped literals are opaque. Dynamic
paths, concatenation, `path.resolve`, computed properties, destructured bindings,
mutable bindings, multi-declarator/typed/semicolon-free const initializers,
cross-module constant propagation, and unsupported import forms grant nothing.
Function/method/catch/arrow parameter names and potentially reassigned bindings
are conservatively invalidated throughout the file. This may reject an otherwise
valid writer in another scope. Duplicate or unresolved declarations also grant
nothing. These false negatives retain the protected-source failure gate.

## Observation and evidence

The existing isolated preflight, acceptance decision, and event schema remain in
use. `recovery_observation_effect_policy_bound.allowed_generated_paths` records
the same array field with exact paths now available before lazy initialization.
Observed business failures must remain `Failed`; permission to generate JSON is
not evidence of business success. Source mutation and unregistered additions
still yield `Unavailable`. Non-runtime symlinks are rejected during snapshot
collection rather than silently omitted from the protected hash.

Tests use source-only synthetic corpus fixtures and synthetic data. The original
S2 storage source was read for its import/local-variable shape only; no original
runtime data is copied or changed. Regex/template fixtures are intentionally
excluded syntax, not examples of supported writers. Final verification results
are recorded separately after execution.

The executable GET fixture is plain JavaScript (Node ESM, filesystem promises,
and a plain status/body response). Type stripping and Fetch globals are not
required. Typed S2-style storage remains a separate static recognizer fixture;
no CI gate or test is skipped to accommodate Node versions.
