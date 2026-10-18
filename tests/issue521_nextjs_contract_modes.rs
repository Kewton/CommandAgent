//! Pins the Next.js final and invariant verification outputs for a table of
//! missing / partial / invalid / complete workspaces, so a structural move of
//! the shared contract checks cannot change a verdict or a reason string.
//! Only the public entry points are used.

use std::path::Path;

use commandagent::planner::profile::{
    ProfileSnapshot, verify_profile_final, verify_profile_invariant,
};
use commandagent::planner::profiles::nextjs;
use commandagent::planner::verify::VerificationReport;

const GOAL: &str = "create a todo app";
const PAGE: &str = "export default function Page() {\n  return <main>ready</main>;\n}\n";
const LAYOUT: &str = "export default function Layout({ children }) {\n  return <html lang=\"en\"><body>{children}</body></html>;\n}\n";
const ENTRYPOINT_MISSING: &str = "Next entrypoint missing: expected src/app/page.tsx, src/app/page.jsx, src/app/page.ts, src/app/page.js, app/page.tsx, app/page.jsx, app/page.ts, app/page.js, pages/index.tsx, pages/index.jsx, pages/index.ts, pages/index.js, src/pages/index.tsx, src/pages/index.jsx, src/pages/index.ts, src/pages/index.js";

fn package(deps: &str, scripts: &str) -> String {
    format!("{{\"name\":\"t\",\"dependencies\":{{{deps}}},\"scripts\":{{{scripts}}}}}")
}

const DEPS: &str = "\"next\":\"16.3.1\",\"react\":\"19.2.8\",\"react-dom\":\"19.2.8\"";
const SCRIPTS: &str =
    "\"build\":\"next build\",\"dev\":\"next dev -p 3011\",\"start\":\"next start -p 3011\"";

const REQUEST_MISMATCH_PAGE: &str = "\"use client\";\nasync function postJson(url, init) {\n  const response = await fetch(url, init);\n  if (!response.ok) {\n    throw new Error(\"bad\");\n  }\n  return response.json();\n}\nexport default function Page() {\n  const save = async () => {\n    await postJson(\"/api/todos\", { method: \"POST\", body: JSON.stringify({ title: \"x\" }) });\n  };\n  return <main onClick={save}>ready</main>;\n}\n";
const REQUEST_MISMATCH_ROUTE: &str = "export async function POST(request) {\n  const body = await request.json();\n  const { text } = body;\n  if (text !== undefined) {\n    return Response.json({ item: 1 });\n  }\n  return Response.json({ error: \"x\" }, { status: 400 });\n}\n";
const ALIAS_PAGE: &str = "const alias = \"@/lib/x\";\nexport default function Page() {\n  return <main>{alias}</main>;\n}\n";
const TS_DEPS: &str = "\"next\":\"16.3.1\",\"react\":\"19.2.8\",\"react-dom\":\"19.2.8\",\"typescript\":\"^5.5.0\",\"@types/react\":\"^19.0.0\",\"@types/react-dom\":\"^19.0.0\"";
const TS_ALIAS_CONFIG: &str =
    "{\"compilerOptions\":{\"baseUrl\":\".\",\"paths\":{\"@/*\":[\"./*\"]}}}";

fn ts_package() -> String {
    package(TS_DEPS, SCRIPTS)
}

fn ok_package() -> String {
    package(DEPS, SCRIPTS)
}

fn with_scripts(scripts: &str) -> String {
    package(DEPS, scripts)
}

/// Expected verdict: `None` is a pass, `Some(reason)` is one profile failure.
type Expect = Option<&'static str>;

struct Case {
    name: &'static str,
    files: Vec<(&'static str, Vec<u8>)>,
    final_mode: Expect,
    invariant_mode: Expect,
}

fn text(path: &'static str, body: impl Into<String>) -> (&'static str, Vec<u8>) {
    (path, body.into().into_bytes())
}

/// A complete plain-JavaScript App Router project plus the given overrides.
fn project(overrides: Vec<(&'static str, Vec<u8>)>) -> Vec<(&'static str, Vec<u8>)> {
    let mut files = vec![
        text("package.json", ok_package()),
        text("app/page.js", PAGE),
        text("app/layout.js", LAYOUT),
    ];
    for (path, body) in overrides {
        files.retain(|(existing, _)| *existing != path);
        files.push((path, body));
    }
    files
}

fn write(root: &Path, files: &[(&'static str, Vec<u8>)]) {
    for (rel, body) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }
}

fn expected(reason: Expect) -> VerificationReport {
    match reason {
        None => VerificationReport::pass(),
        Some(reason) => VerificationReport::profile_failed(reason),
    }
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "complete plain js",
            files: project(vec![]),
            final_mode: None,
            invariant_mode: None,
        },
        Case {
            name: "missing: empty workspace",
            files: vec![],
            final_mode: Some("package.json missing"),
            invariant_mode: None,
        },
        Case {
            name: "missing: only an unrelated file",
            files: vec![text("README.md", "hello")],
            final_mode: Some("package.json missing"),
            invariant_mode: None,
        },
        Case {
            name: "invalid: package.json is not json",
            files: project(vec![text("package.json", "{")]),
            final_mode: Some("package.json invalid"),
            invariant_mode: Some("package.json invalid"),
        },
        Case {
            name: "invalid: package.json is not utf-8",
            files: project(vec![("package.json", vec![0xff, 0xfe, 0x00])]),
            final_mode: Some("package.json missing"),
            invariant_mode: Some("package.json unreadable"),
        },
        Case {
            name: "invalid: two nested projects",
            files: vec![
                text("a/package.json", ok_package()),
                text("b/package.json", ok_package()),
            ],
            final_mode: Some(
                "multiple nested package.json files found; keep one Next.js project in the workspace",
            ),
            invariant_mode: Some(
                "multiple nested package.json files found; keep one Next.js project in the workspace",
            ),
        },
        Case {
            name: "complete: nested project",
            files: vec![
                text("web/package.json", ok_package()),
                text("web/app/page.js", PAGE),
                text("web/app/layout.js", LAYOUT),
            ],
            final_mode: None,
            invariant_mode: None,
        },
        Case {
            name: "partial: nested project without entrypoint",
            files: vec![text("web/package.json", ok_package())],
            final_mode: Some(
                "web: Next entrypoint missing: expected src/app/page.tsx, src/app/page.jsx, src/app/page.ts, src/app/page.js, app/page.tsx, app/page.jsx, app/page.ts, app/page.js, pages/index.tsx, pages/index.jsx, pages/index.ts, pages/index.js, src/pages/index.tsx, src/pages/index.jsx, src/pages/index.ts, src/pages/index.js",
            ),
            invariant_mode: None,
        },
        Case {
            name: "partial: dependency next missing",
            files: project(vec![text(
                "package.json",
                package("\"react\":\"19.2.8\",\"react-dom\":\"19.2.8\"", SCRIPTS),
            )]),
            final_mode: Some("dependency missing: next"),
            invariant_mode: Some("dependency missing: next"),
        },
        Case {
            name: "partial: dependency react-dom missing",
            files: project(vec![text(
                "package.json",
                package("\"next\":\"16.3.1\",\"react\":\"19.2.8\"", SCRIPTS),
            )]),
            final_mode: Some("dependency missing: react-dom"),
            invariant_mode: Some("dependency missing: react-dom"),
        },
        Case {
            name: "partial: no scripts at all",
            files: project(vec![text(
                "package.json",
                format!("{{\"name\":\"t\",\"dependencies\":{{{DEPS}}}}}"),
            )]),
            final_mode: Some("scripts.build must be next build"),
            invariant_mode: None,
        },
        Case {
            name: "partial: build missing, dev and start present",
            files: project(vec![text(
                "package.json",
                with_scripts("\"dev\":\"next dev -p 3011\",\"start\":\"next start -p 3011\""),
            )]),
            final_mode: Some("scripts.build must be next build"),
            invariant_mode: None,
        },
        Case {
            name: "partial: build present, dev missing",
            files: project(vec![text(
                "package.json",
                with_scripts("\"build\":\"next build\""),
            )]),
            final_mode: Some("dev script must run next dev on port 3011"),
            invariant_mode: None,
        },
        Case {
            name: "invalid: build is a no-op echo",
            files: project(vec![text(
                "package.json",
                with_scripts("\"build\":\"echo ok\",\"dev\":\"next dev -p 3011\""),
            )]),
            final_mode: Some("scripts.build must be next build"),
            invariant_mode: Some("scripts.build must be next build"),
        },
        Case {
            name: "invalid: build is not next build",
            files: project(vec![text(
                "package.json",
                with_scripts("\"build\":\"tsc\",\"dev\":\"next dev -p 3011\""),
            )]),
            final_mode: Some("scripts.build must be next build"),
            invariant_mode: Some("scripts.build must be next build"),
        },
        Case {
            name: "invalid: dev is a weakened script",
            files: project(vec![text(
                "package.json",
                with_scripts("\"build\":\"next build\",\"dev\":\"echo dev\""),
            )]),
            final_mode: Some("scripts.dev must run next dev"),
            invariant_mode: Some("dev script must run next dev on port 3011"),
        },
        Case {
            name: "invalid: dev on the wrong port",
            files: project(vec![text(
                "package.json",
                with_scripts("\"build\":\"next build\",\"dev\":\"next dev -p 4000\""),
            )]),
            final_mode: Some("dev script must run next dev on port 3011"),
            invariant_mode: Some("dev script must run next dev on port 3011"),
        },
        Case {
            name: "invalid: start on the wrong port",
            files: project(vec![text(
                "package.json",
                with_scripts(
                    "\"build\":\"next build\",\"dev\":\"next dev -p 3011\",\"start\":\"next start -p 4000\"",
                ),
            )]),
            final_mode: Some("start script must run next start on port 3011"),
            invariant_mode: Some("start script must run next start on port 3011"),
        },
        Case {
            name: "complete: start script absent",
            files: project(vec![text(
                "package.json",
                with_scripts("\"build\":\"next build\",\"dev\":\"next dev -p 3011\""),
            )]),
            final_mode: None,
            invariant_mode: None,
        },
        Case {
            name: "partial: package only, no entrypoint",
            files: vec![text("package.json", ok_package())],
            final_mode: Some(ENTRYPOINT_MISSING),
            invariant_mode: None,
        },
        Case {
            name: "partial: entrypoint without layout",
            files: vec![
                text("package.json", ok_package()),
                text("app/page.js", PAGE),
            ],
            final_mode: Some(
                "Next app router layout missing: expected app/layout.tsx or layout.jsx",
            ),
            invariant_mode: None,
        },
        Case {
            name: "invalid: use of @/ alias without tsconfig",
            files: project(vec![text("app/page.jsx", ALIAS_PAGE)]),
            final_mode: Some("tsconfig.json missing for @/* alias"),
            invariant_mode: None,
        },
        Case {
            name: "invalid: alias with unparsable tsconfig",
            files: project(vec![
                text("package.json", ts_package()),
                text("app/page.jsx", ALIAS_PAGE),
                text("tsconfig.json", "{"),
            ]),
            final_mode: Some("tsconfig.json invalid"),
            invariant_mode: None,
        },
        Case {
            name: "invalid: alias with tsconfig lacking the alias",
            files: project(vec![
                text("package.json", ts_package()),
                text("app/page.jsx", ALIAS_PAGE),
                text("tsconfig.json", "{\"compilerOptions\":{}}"),
            ]),
            final_mode: Some("tsconfig baseUrl/paths missing @/* alias"),
            invariant_mode: None,
        },
        Case {
            name: "complete: alias with configured tsconfig",
            files: project(vec![
                text("package.json", ts_package()),
                text("app/page.jsx", ALIAS_PAGE),
                text("tsconfig.json", TS_ALIAS_CONFIG),
            ]),
            final_mode: None,
            invariant_mode: None,
        },
        Case {
            name: "invalid: tsconfig moduleResolution node10",
            files: project(vec![
                text("package.json", ts_package()),
                text(
                    "tsconfig.json",
                    "{\"compilerOptions\":{\"moduleResolution\":\"node10\"}}",
                ),
            ]),
            final_mode: Some(
                "tsconfig.moduleResolution=node10 is deprecated for Next.js builds; use bundler or node16",
            ),
            invariant_mode: Some(
                "tsconfig.moduleResolution=node10 is deprecated for Next.js builds; use bundler or node16",
            ),
        },
        Case {
            name: "invalid: tsconfig rootDir constrains output",
            files: project(vec![
                text("package.json", ts_package()),
                text(
                    "tsconfig.json",
                    "{\"compilerOptions\":{\"rootDir\":\"src\"}}",
                ),
            ]),
            final_mode: Some("tsconfig.rootDir must not constrain Next.js generated files"),
            invariant_mode: Some("tsconfig.rootDir must not constrain Next.js generated files"),
        },
        Case {
            name: "invalid: typescript project without typescript dependency",
            files: project(vec![text("tsconfig.json", "{\"compilerOptions\":{}}")]),
            final_mode: Some(
                "typescript dependency must use a deterministic 5.x range such as ^5.5.0",
            ),
            invariant_mode: Some(
                "typescript dependency must use a deterministic 5.x range such as ^5.5.0",
            ),
        },
        Case {
            name: "invalid: css side-effect import without declaration",
            files: project(vec![
                text("package.json", ts_package()),
                text("tsconfig.json", "{\"compilerOptions\":{}}"),
                text(
                    "app/layout.tsx",
                    "import \"./globals.css\";\nexport default function Layout({ children }: { children: React.ReactNode }) {\n  return <html lang=\"en\"><body>{children}</body></html>;\n}\n",
                ),
                text("app/globals.css", "body { margin: 0; }\n"),
            ]),
            final_mode: Some(
                "CSS side-effect imports require a declaration file such as src/app/global.d.ts with declare module \"*.css\"",
            ),
            invariant_mode: Some(
                "CSS side-effect imports require a declaration file such as src/app/global.d.ts with declare module \"*.css\"",
            ),
        },
        Case {
            name: "final only: request field mismatch against the api route",
            files: project(vec![
                text("app/page.js", REQUEST_MISMATCH_PAGE),
                text("app/api/todos/route.js", REQUEST_MISMATCH_ROUTE),
            ]),
            final_mode: Some(
                "api_contract_failure: POST /api/todos request field mismatch: app/page.js sends {\"title\"}, but app/api/todos/route.js reads {\"text\"}; align request, shared types and stored value, then verify the committed result after reload",
            ),
            invariant_mode: None,
        },
        Case {
            name: "invalid: client mutation without a matching api route",
            files: project(vec![text(
                "app/page.js",
                "\"use client\";\nexport default function Page() {\n  const save = () => fetch(\"/api/todos\", { method: \"POST\" });\n  return <main onClick={save}>ready</main>;\n}\n",
            )]),
            final_mode: Some(
                "api_contract_failure: POST /api/todos used by app/page.js has no matching App Router route",
            ),
            invariant_mode: Some(
                "api_contract_failure: POST /api/todos used by app/page.js has no matching App Router route",
            ),
        },
        Case {
            name: "invalid: client component without use client",
            files: project(vec![text(
                "app/page.js",
                "import { useState } from \"react\";\nexport default function Page() {\n  const [n] = useState(0);\n  return <main>{n}</main>;\n}\n",
            )]),
            final_mode: Some(
                "app/page.js uses browser/client APIs and must start with \"use client\"",
            ),
            invariant_mode: Some(
                "app/page.js uses browser/client APIs and must start with \"use client\"",
            ),
        },
        Case {
            name: "invalid: missing relative import",
            files: project(vec![text(
                "app/page.js",
                "import x from \"./missing\";\nexport default function Page() {\n  return <main>{x}</main>;\n}\n",
            )]),
            final_mode: Some(
                "missing relative imports: app/page.js imports ./missing which does not exist - create app/missing",
            ),
            invariant_mode: Some(
                "missing relative imports: app/page.js imports ./missing which does not exist - create app/missing",
            ),
        },
        Case {
            name: "invalid: nested project missing relative import",
            files: vec![
                text("web/package.json", ok_package()),
                text(
                    "web/app/page.js",
                    "import x from \"./missing\";\nexport default function Page() {\n  return <main>{x}</main>;\n}\n",
                ),
                text("web/app/layout.js", LAYOUT),
            ],
            final_mode: Some(
                "web: missing relative imports: app/page.js imports ./missing which does not exist - create app/missing",
            ),
            invariant_mode: Some(
                "web: missing relative imports: app/page.js imports ./missing which does not exist - create app/missing",
            ),
        },
        Case {
            name: "invalid: tailwind without postcss",
            files: project(vec![text(
                "package.json",
                package(&format!("{DEPS},\"tailwindcss\":\"3.4.0\""), SCRIPTS),
            )]),
            final_mode: Some(
                "tailwind_contract_failure: Tailwind toolchain dependency missing: postcss",
            ),
            invariant_mode: Some(
                "tailwind_contract_failure: Tailwind toolchain dependency missing: postcss",
            ),
        },
    ]
}

#[test]
fn nextjs_contract_modes_table_matches_baseline() {
    let mut mismatches = Vec::new();
    for case in cases() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), &case.files);
        let final_report = nextjs::verify(dir.path(), GOAL);
        let invariant_report = nextjs::verify_invariant(dir.path(), GOAL);
        if final_report != expected(case.final_mode) {
            mismatches.push(format!(
                "[final] {}: expected {:?}, got {:?}",
                case.name, case.final_mode, final_report.profile_failures
            ));
        }
        if invariant_report != expected(case.invariant_mode) {
            mismatches.push(format!(
                "[invariant] {}: expected {:?}, got {:?}",
                case.name, case.invariant_mode, invariant_report.profile_failures
            ));
        }
        assert_eq!(
            verify_profile_final(dir.path(), "nextjs", GOAL),
            final_report,
            "{}: DomainProfile final wrapper diverged",
            case.name
        );
        assert_eq!(
            verify_profile_invariant(dir.path(), "nextjs", GOAL, &ProfileSnapshot::None),
            invariant_report,
            "{}: DomainProfile invariant wrapper diverged",
            case.name
        );
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
