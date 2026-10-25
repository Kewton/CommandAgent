//! Entry-reachability check for Next.js App Router page routes (IC-30).
//!
//! The browser probe only opens `/`, and "route bound" means "under
//! `src/app`", not "reachable from `/`". A page that carries the contract hook
//! (`data-anvil-action`) but that no UI element links to would therefore not
//! stop a final acceptance from being `full`. This leaf statically follows the
//! navigations from the entry (`/` page, its layouts, and their import
//! closures) and reports every hook-carrying page route it cannot reach.
//!
//! The check only ever lowers a release gate (`pass` -> `partial` -> `failed`);
//! no outcome of it raises one.
//!
//! Route handling:
//!
//! | shape | handling |
//! |---|---|
//! | `app/**/page.{ts,tsx,js,jsx}` | page route; `(group)` and `@slot` segments are dropped |
//! | page under an `api` segment, `_private` folder, or `(.)` interception | not a target |
//! | `/` page (after dropping groups) | entry, never a target |
//! | page without `data-anvil-action` in its import closure | not a target |
//! | `[id]`, `[...slug]`, `[[...slug]]` | matches one segment, one or more, zero or more |
//!
//! Navigation forms (`syntax` in the evidence) and how their argument is read:
//!
//! | form | argument shapes read as a route |
//! |---|---|
//! | `href=`, `<Link href=>` | `"/p"`, `{"/p"}`, `` {`/p/${x}`} ``, `{"/p/" + x}`, `{cond ? "/a" : "/b"}`, `{{ pathname: "/p/[id]" }}` |
//! | `router.push/replace(..)`, `redirect(..)`, `permanentRedirect(..)` | same |
//! | `location.href = ..`, `location.assign/replace(..)`, `window.open(..)` | same |
//! | `<form action=..>` | string or template literal only (a function is not a navigation) |
//!
//! Argument classification:
//!
//! | argument | result |
//! |---|---|
//! | absolute path literal (query and hash dropped; `${..}` and `[id]` match one segment) | compared with every page route |
//! | absolute path that starts with a base expression (`${base}/notes`) | the base may stand for leading route segments (`[locale]`); a match that needs skipped segments is never a reach, it makes that route unverifiable (`leading_base_expression:<route>`) |
//! | `http:`, `mailto:`, `#..`, `?..`, `//host`, empty | not a route navigation, ignored |
//! | `href="$1"` (only `$n`/`$&` replacement references) written inside a JS string | not a navigation, ignored (never unresolved) |
//! | relative path (`notes/1`, `./x`) | unresolved |
//! | variable, call, member access, a literal-free template | unresolved |
//!
//! One rule decides a target route:
//!
//! | situation | outcome |
//! |---|---|
//! | a recognized navigation matches the route | reached (`pass`) |
//! | no navigation matches, but the check cannot rule one out (below) | `route_reachability_unverified:<route>` (`partial`) |
//! | nothing matches and nothing is unverifiable | `route_unreachable_from_entry:<route>` (`failed`) |
//!
//! Unverifiable means any of: a navigation whose argument is not static; a
//! route-shaped string literal in the reachable closure that matches a page
//! route (a constant, helper, or link table) or a link that needs skipped
//! leading segments (`leading_base_expression`); each of these two makes only
//! that route unverifiable, and the closure of such a route is followed as
//! "may be reached": a navigation, literal, or leading-base link found there
//! makes its route unverifiable too, never reached, and a reason below found
//! only there makes the other unreached routes unverifiable; a router alias (`const nav =
//! useRouter()`, `const { push } = useRouter()`, `redirect as go`); an import
//! the scan cannot follow (`~/`, `#`, a tsconfig `paths` alias, or a `.mdx`,
//! `.mjs`, `.cjs` file); `middleware.*`, or a `next.config.*` with `rewrites`
//! or `redirects`; the scan file cap. Every one of them only turns `failed`
//! into `partial`; none of them reaches a route.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::Context;
use regex::Regex;
use serde::Serialize;

use crate::evidence_envelope::{EvidenceEnvelopeSpec, EvidenceFamily};
use crate::minimal_loop::import_scan::{
    extract_import_specifiers, is_relative_specifier, nextjs_project_root, normalize_pathbuf,
    resolve_route_import, resolve_workspace_alias_import,
};

pub(crate) const CHECK_ID: &str = "nextjs_route_reachability";
pub(crate) const EVIDENCE_RELATIVE_PATH: &str =
    ".commandagent/evidence/nextjs-route-reachability.json";

const HOOK_ATTRIBUTE: &str = "data-anvil-action";
const UNREACHABLE_REASON: &str = "route_unreachable_from_entry";
const UNREACHABLE_REPAIR: &str =
    "add a link from the entry page to this route, or delete the page if it is not used";
const UNVERIFIED_REASON: &str = "route_reachability_unverified";
const MAX_CLOSURE_FILES: usize = 2000;
const MAX_EVIDENCE_UNRESOLVED: usize = 50;
const SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx"];
const PAGE_AND_LAYOUT_EXTENSIONS: &[&str] = &["tsx", "ts", "jsx", "js"];
/// Stands for a `${..}` or a non-literal operand inside a flattened pattern.
const WILD: char = '\u{1}';

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReachabilityStatus {
    #[default]
    Pass,
    Partial,
    Failed,
}

impl ReachabilityStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Transition {
    pub file: String,
    pub line: usize,
    pub syntax: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct TargetEvidence {
    pub route: String,
    pub page: String,
    pub hook_source: String,
    pub outcome: &'static str,
    pub reached_via: Option<Transition>,
    /// What to change when the page is unreachable; absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct UnresolvedNavigation {
    pub file: String,
    pub line: usize,
    pub syntax: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub(crate) struct ReachabilityReport {
    pub check_id: &'static str,
    pub status: ReachabilityStatus,
    pub entry_sources: Vec<String>,
    pub reached_routes: Vec<String>,
    pub targets: Vec<TargetEvidence>,
    pub unresolved_navigations: Vec<UnresolvedNavigation>,
    pub scan_truncated: bool,
    pub unverifiable: Vec<String>,
    pub violations: Vec<String>,
    pub unverified: Vec<String>,
}

impl ReachabilityReport {
    /// Violations first, then unverified routes.
    pub(crate) fn reasons(&self) -> Vec<String> {
        self.violations
            .iter()
            .chain(&self.unverified)
            .cloned()
            .collect()
    }
}

/// Combines the testimony binding result with the reachability result. The
/// result is the worst of the two (`failed` > `partial` > `pass`); nothing here
/// can produce a better status than either input.
pub(crate) fn merge_with_testimony(
    testimony_failed: bool,
    testimony_violations: Vec<String>,
    report: &ReachabilityReport,
) -> (&'static str, Vec<String>) {
    let status = if testimony_failed || report.status == ReachabilityStatus::Failed {
        ReachabilityStatus::Failed
    } else {
        report.status
    };
    let mut reasons = testimony_violations;
    reasons.extend(report.reasons());
    (status.as_str(), reasons)
}

pub(crate) fn write_evidence(root: &Path, report: &ReachabilityReport) -> anyhow::Result<()> {
    let path = root.join(EVIDENCE_RELATIVE_PATH);
    let parent = path
        .parent()
        .context("route reachability evidence path must have a parent")?;
    std::fs::create_dir_all(parent)?;
    let sources = report
        .entry_sources
        .iter()
        .chain(report.targets.iter().map(|target| &target.page));
    crate::evidence_envelope::write_json(
        &path,
        report,
        EvidenceEnvelopeSpec::new(EvidenceFamily::T, "route_reachability")
            .with_source_refs(sources),
        true,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Static(String),
    Dynamic,
    CatchAll,
    OptionalCatchAll,
}

#[derive(Debug, Clone)]
struct Page {
    route: String,
    segments: Vec<Segment>,
    file: PathBuf,
    /// Layouts and templates that wrap the page, root first.
    wrappers: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NavTarget {
    Route(String),
    Unresolved,
    Ignore,
}

#[derive(Debug, Clone)]
struct Navigation {
    file: String,
    line: usize,
    syntax: &'static str,
    target: NavTarget,
}

#[derive(Debug, Clone, Default)]
struct FileInfo {
    has_hook: bool,
    navigations: Vec<Navigation>,
    literals: Vec<(usize, String)>,
    imports: Vec<PathBuf>,
    unverifiable: Vec<String>,
}

struct Scanner<'a> {
    root: &'a Path,
    project_root: PathBuf,
    cache: BTreeMap<PathBuf, FileInfo>,
    truncated: bool,
    /// `paths` keys of tsconfig/jsconfig (`*` stripped) and whether exact.
    path_aliases: Vec<(String, bool)>,
}

pub(crate) fn evaluate(root: &Path) -> ReachabilityReport {
    let mut report = ReachabilityReport {
        check_id: CHECK_ID,
        ..ReachabilityReport::default()
    };
    let Some((project_root, _prefix)) = nextjs_project_root(root) else {
        return report;
    };
    let mut scanner = Scanner {
        root,
        cache: BTreeMap::new(),
        truncated: false,
        path_aliases: tsconfig_path_aliases(&project_root),
        project_root,
    };
    let pages = scanner.enumerate_pages();
    let entry_pages: Vec<&Page> = pages
        .iter()
        .filter(|page| page.segments.is_empty())
        .collect();
    if entry_pages.is_empty() {
        return report;
    }

    let mut starts = Vec::new();
    for page in &entry_pages {
        starts.push(page.file.clone());
        starts.extend(page.wrappers.iter().cloned());
    }
    report.entry_sources = starts.iter().map(|path| display(path)).collect();
    let mut origin = scanner.closure(starts);

    let mut reached: BTreeMap<usize, Transition> = BTreeMap::new();
    loop {
        let navigations = scanner.navigations(&origin);
        let literals = scanner.literals(&origin);
        let has_unresolved = navigations
            .iter()
            .any(|nav| nav.target == NavTarget::Unresolved);
        let mut new_starts = Vec::new();
        for (index, page) in pages.iter().enumerate() {
            if page.segments.is_empty() || reached.contains_key(&index) {
                continue;
            }
            if let Some(via) = find_navigation(page, &navigations) {
                reached.insert(index, via);
                new_starts.push(page.file.clone());
                new_starts.extend(page.wrappers.iter().cloned());
            }
        }
        if new_starts.is_empty() {
            report.unresolved_navigations = navigations
                .iter()
                .filter(|nav| nav.target == NavTarget::Unresolved)
                .take(MAX_EVIDENCE_UNRESOLVED)
                .map(|nav| UnresolvedNavigation {
                    file: nav.file.clone(),
                    line: nav.line,
                    syntax: nav.syntax,
                })
                .collect();
            report.scan_truncated = scanner.truncated;
            let mut reasons = scanner.unverifiable(&origin);
            if has_unresolved {
                reasons.push("unresolved_navigation".to_string());
            }
            if scanner.truncated {
                reasons.push("scan_truncated".to_string());
            }
            reasons.sort();
            reasons.dedup();
            let mut everywhere = !reasons.is_empty();
            // Routes that may be reached (never `reached`): the pages whose
            // route-shaped literal or leading-base link matched, then
            // everything their own closures point to, until nothing new.
            let mut maybe: BTreeSet<usize> = BTreeSet::new();
            let mut maybe_files: BTreeSet<PathBuf> = BTreeSet::new();
            let (mut hit_navigations, mut hit_literals) = (navigations.clone(), literals.clone());
            loop {
                let mut added = Vec::new();
                for (index, page) in pages.iter().enumerate() {
                    if page.segments.is_empty()
                        || reached.contains_key(&index)
                        || maybe.contains(&index)
                    {
                        continue;
                    }
                    let causes = [
                        (
                            "maybe_reached",
                            find_navigation(page, &hit_navigations).is_some(),
                        ),
                        (
                            "leading_base_expression",
                            leading_base_hit(page, &hit_navigations),
                        ),
                        ("route_shaped_literal", literal_hit(page, &hit_literals)),
                    ];
                    let mut hit = false;
                    for (cause, found) in causes {
                        if found {
                            reasons.push(format!("{cause}:{}", page.route));
                            hit = true;
                        }
                    }
                    if hit {
                        added.push(index);
                    }
                }
                if added.is_empty() {
                    break;
                }
                let mut starts = Vec::new();
                for index in &added {
                    starts.push(pages[*index].file.clone());
                    starts.extend(pages[*index].wrappers.iter().cloned());
                }
                maybe.extend(added);
                maybe_files.extend(scanner.closure(starts));
                hit_navigations = scanner.navigations(&maybe_files);
                hit_literals = scanner.literals(&maybe_files);
            }
            // Reasons found only beyond the entry's certain closure leave the
            // routes past that stage unverifiable; they never reach anything.
            let beyond: BTreeSet<PathBuf> = maybe_files.difference(&origin).cloned().collect();
            let mut stage = scanner.unverifiable(&beyond);
            if scanner
                .navigations(&beyond)
                .iter()
                .any(|nav| nav.target == NavTarget::Unresolved)
            {
                stage.push("unresolved_navigation".to_string());
            }
            if scanner.truncated && !report.scan_truncated {
                stage.push("scan_truncated".to_string());
            }
            everywhere |= !stage.is_empty();
            reasons.extend(stage);
            reasons.sort();
            reasons.dedup();
            report.unverifiable = reasons;
            let unverified = Unverified {
                everywhere,
                routes: maybe
                    .iter()
                    .map(|index| pages[*index].route.clone())
                    .collect(),
            };
            record_targets(&mut report, &mut scanner, &pages, &reached, &unverified);
            break;
        }
        origin.extend(scanner.closure(new_starts));
    }
    report.reached_routes = reached
        .keys()
        .map(|index| pages[*index].route.clone())
        .collect();
    report.status = if !report.violations.is_empty() {
        ReachabilityStatus::Failed
    } else if !report.unverified.is_empty() {
        ReachabilityStatus::Partial
    } else {
        ReachabilityStatus::Pass
    };
    report
}

fn record_targets(
    report: &mut ReachabilityReport,
    scanner: &mut Scanner<'_>,
    pages: &[Page],
    reached: &BTreeMap<usize, Transition>,
    unverified: &Unverified,
) {
    for (index, page) in pages.iter().enumerate() {
        if page.segments.is_empty() {
            continue;
        }
        let page_closure = scanner.closure(vec![page.file.clone()]);
        let Some(hook_source) = page_closure
            .iter()
            .find(|file| scanner.info(file).has_hook)
            .map(|file| display(file))
        else {
            continue;
        };
        let via = reached.get(&index).cloned();
        let mut repair = None;
        let outcome = if via.is_some() {
            "reached"
        } else if unverified.everywhere || unverified.routes.contains(&page.route) {
            report
                .unverified
                .push(format!("{UNVERIFIED_REASON}:{}", page.route));
            "unverified"
        } else {
            report
                .violations
                .push(format!("{UNREACHABLE_REASON}:{}", page.route));
            repair = Some(UNREACHABLE_REPAIR);
            "unreachable"
        };
        report.targets.push(TargetEvidence {
            route: page.route.clone(),
            page: display(&page.file),
            hook_source,
            outcome,
            reached_via: via,
            repair,
        });
    }
}

struct Unverified {
    /// A reason that leaves every unreached target unverifiable.
    everywhere: bool,
    /// Routes made unverifiable by a reason that concerns only them.
    routes: BTreeSet<String>,
}

fn find_navigation(page: &Page, navigations: &[Navigation]) -> Option<Transition> {
    navigations.iter().find_map(|nav| match &nav.target {
        NavTarget::Route(link) if link_matches(link, &page.segments, false) => Some(Transition {
            file: nav.file.clone(),
            line: nav.line,
            syntax: nav.syntax,
        }),
        _ => None,
    })
}

/// A link that starts with a base expression and matches only after skipping
/// leading route segments. The base is not statically known, so it is never a
/// reach.
fn leading_base_hit(page: &Page, navigations: &[Navigation]) -> bool {
    navigations.iter().any(|nav| match &nav.target {
        NavTarget::Route(link) => link_matches(link, &page.segments, true),
        _ => false,
    })
}

fn literal_hit(page: &Page, literals: &[(String, usize, String)]) -> bool {
    literals.iter().any(|(_, _, text)| {
        let Some(link) = route_shaped(text) else {
            return false;
        };
        // A constant such as "/notes/" is a prefix the caller completes.
        let link = if link.len() > 1 && link.ends_with('/') {
            format!("{link}{WILD}")
        } else {
            link
        };
        link_matches(&link, &page.segments, true)
    })
}

fn display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

impl Scanner<'_> {
    fn info(&mut self, rel: &Path) -> &FileInfo {
        if !self.cache.contains_key(rel) {
            let info = self.read_info(rel);
            self.cache.insert(rel.to_path_buf(), info);
        }
        &self.cache[rel]
    }

    fn unfollowable_import(&self, specifier: &str) -> Option<&'static str> {
        let lower = specifier.to_ascii_lowercase();
        if [".mdx", ".mjs", ".cjs"]
            .iter()
            .any(|ext| lower.ends_with(ext))
        {
            return Some("unread_import");
        }
        if specifier.starts_with("~/") || specifier.starts_with('#') {
            return Some("unresolved_import");
        }
        self.path_aliases
            .iter()
            .any(|(alias, exact)| {
                if *exact {
                    specifier == alias
                } else {
                    specifier.starts_with(alias.as_str())
                }
            })
            .then_some("unresolved_import")
    }

    /// Reasons the reachable files (and the project) leave navigation unverifiable.
    fn unverifiable(&mut self, files: &BTreeSet<PathBuf>) -> Vec<String> {
        let mut out = Vec::new();
        for file in files {
            out.extend(self.info(file).unverifiable.iter().cloned());
        }
        out.extend(self.project_rewrites());
        out
    }

    fn project_rewrites(&self) -> Vec<String> {
        let mut out = Vec::new();
        for dir in [self.project_root.clone(), self.project_root.join("src")] {
            for ext in ["ts", "js", "tsx", "jsx", "mjs", "cjs"] {
                if dir.join(format!("middleware.{ext}")).is_file() {
                    out.push("middleware".to_string());
                }
            }
        }
        for ext in ["js", "mjs", "cjs", "ts", "mts"] {
            let path = self.project_root.join(format!("next.config.{ext}"));
            if let Ok(text) = std::fs::read_to_string(path) {
                let code = blank_comments(&text);
                if code.contains("rewrites") || code.contains("redirects") {
                    out.push("next_config_rewrites".to_string());
                }
            }
        }
        out
    }

    fn read_info(&self, rel: &Path) -> FileInfo {
        let is_source = rel
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| SOURCE_EXTENSIONS.contains(&ext));
        let full = self.root.join(rel);
        if !is_source || !full.is_file() {
            return FileInfo::default();
        }
        let Ok(raw) = std::fs::read_to_string(&full) else {
            return FileInfo::default();
        };
        let code = blank_comments(&raw);
        let file = display(rel);
        let mut info = FileInfo {
            has_hook: raw.contains(HOOK_ATTRIBUTE),
            ..FileInfo::default()
        };
        info.navigations = scan_navigations(&code, &file);
        if ROUTER_ALIAS.iter().any(|regex| {
            regex.captures_iter(&code).any(|found| {
                found
                    .get(1)
                    .is_none_or(|name| !name.as_str().to_ascii_lowercase().contains("router"))
            })
        }) {
            info.unverifiable.push(format!("router_alias:{file}"));
        }
        info.literals = string_literals(&code)
            .into_iter()
            .map(|(offset, text)| (line_of(&code, offset), text))
            .collect();
        if let Some(parent) = full.parent() {
            for specifier in extract_import_specifiers(&raw) {
                if let Some(reason) = self.unfollowable_import(&specifier) {
                    info.unverifiable
                        .push(format!("{reason}:{file}:{specifier}"));
                }
                let candidates = if is_relative_specifier(&specifier) {
                    resolve_route_import(parent, &specifier)
                } else if let Some(alias) = specifier.strip_prefix("@/") {
                    resolve_workspace_alias_import(&self.project_root, alias)
                } else {
                    Vec::new()
                };
                let resolved = candidates.into_iter().find(|candidate| candidate.is_file());
                if let Some(resolved) = resolved
                    && let Ok(rel) = resolved.strip_prefix(self.root)
                {
                    info.imports.push(normalize_pathbuf(rel));
                }
            }
        }
        info
    }

    /// Import closure of `starts` (relative to the workspace root), `starts`
    /// included.
    fn closure(&mut self, starts: Vec<PathBuf>) -> BTreeSet<PathBuf> {
        let mut seen = BTreeSet::new();
        let mut stack = starts;
        while let Some(rel) = stack.pop() {
            if seen.contains(&rel) {
                continue;
            }
            if seen.len() >= MAX_CLOSURE_FILES {
                self.truncated = true;
                break;
            }
            seen.insert(rel.clone());
            stack.extend(self.info(&rel).imports.iter().cloned());
        }
        seen
    }

    fn navigations(&mut self, files: &BTreeSet<PathBuf>) -> Vec<Navigation> {
        let mut out = Vec::new();
        for file in files {
            out.extend(self.info(file).navigations.iter().cloned());
        }
        out
    }

    fn literals(&mut self, files: &BTreeSet<PathBuf>) -> Vec<(String, usize, String)> {
        let mut out = Vec::new();
        for file in files {
            let name = display(file);
            let info = self.info(file);
            out.extend(
                info.literals
                    .iter()
                    .map(|(line, text)| (name.clone(), *line, text.clone())),
            );
        }
        out
    }

    fn enumerate_pages(&self) -> Vec<Page> {
        let mut pages = Vec::new();
        for app_dir in ["src/app", "app"] {
            let app_abs = self.project_root.join(app_dir);
            if !app_abs.is_dir() {
                continue;
            }
            let mut stack = vec![app_abs.clone()];
            while let Some(dir) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Ok(file_type) = entry.file_type() else {
                        continue;
                    };
                    if file_type.is_dir() {
                        stack.push(path);
                    } else if file_type.is_file() && is_page_file(&path) {
                        pages.extend(self.page_for(&app_abs, &path));
                    }
                }
            }
        }
        pages.sort_by(|a, b| (&a.route, &a.file).cmp(&(&b.route, &b.file)));
        pages
    }

    fn page_for(&self, app_abs: &Path, page_abs: &Path) -> Option<Page> {
        let dir = page_abs.parent()?;
        let relative = dir.strip_prefix(app_abs).ok()?;
        let mut segments = Vec::new();
        let mut names = Vec::new();
        let mut wrappers = Vec::new();
        let mut current = app_abs.to_path_buf();
        self.push_wrappers(&current, &mut wrappers);
        for component in relative.components() {
            let name = component.as_os_str().to_str()?;
            current.push(name);
            self.push_wrappers(&current, &mut wrappers);
            if name.starts_with('_') || name.starts_with("(.") {
                return None;
            }
            if name.starts_with('@') || (name.starts_with('(') && name.ends_with(')')) {
                continue;
            }
            names.push(name.to_string());
            segments.push(parse_segment(name));
        }
        if names.first().is_some_and(|name| name == "api") {
            return None;
        }
        let file = normalize_pathbuf(page_abs.strip_prefix(self.root).ok()?);
        Some(Page {
            route: format!("/{}", names.join("/")),
            segments,
            file,
            wrappers,
        })
    }

    fn push_wrappers(&self, dir: &Path, out: &mut Vec<PathBuf>) {
        for stem in ["layout", "template"] {
            for ext in PAGE_AND_LAYOUT_EXTENSIONS {
                let candidate = dir.join(format!("{stem}.{ext}"));
                if candidate.is_file()
                    && let Ok(rel) = candidate.strip_prefix(self.root)
                {
                    out.push(normalize_pathbuf(rel));
                    break;
                }
            }
        }
    }
}

fn is_page_file(path: &Path) -> bool {
    let stem = path.file_stem().and_then(|stem| stem.to_str());
    let ext = path.extension().and_then(|ext| ext.to_str());
    stem == Some("page") && ext.is_some_and(|ext| PAGE_AND_LAYOUT_EXTENSIONS.contains(&ext))
}

fn parse_segment(name: &str) -> Segment {
    if name.starts_with("[[...") && name.ends_with("]]") {
        Segment::OptionalCatchAll
    } else if name.starts_with("[...") && name.ends_with(']') {
        Segment::CatchAll
    } else if name.starts_with('[') && name.ends_with(']') {
        Segment::Dynamic
    } else {
        Segment::Static(name.to_string())
    }
}

/// Matches a flattened link (`WILD` marks unknown text) against a page route.
fn link_matches(link: &str, route: &[Segment], skip_leading: bool) -> bool {
    let end = link.find(['?', '#']).unwrap_or(link.len());
    let path = &link[..end];
    let (leading_any, path) = match path.strip_prefix(WILD) {
        Some(rest) if rest.starts_with('/') => (true, rest),
        _ => (false, path),
    };
    let mut segments: Vec<&str> = path.split('/').skip(1).collect();
    while segments.last() == Some(&"") {
        segments.pop();
    }
    if leading_any && skip_leading {
        (0..=route.len()).any(|skip| segments_match(&segments, &route[skip..]))
    } else {
        segments_match(&segments, route)
    }
}

fn segments_match(link: &[&str], route: &[Segment]) -> bool {
    match route.split_first() {
        None => link.is_empty(),
        Some((Segment::OptionalCatchAll, _)) => true,
        Some((Segment::CatchAll, _)) => !link.is_empty(),
        Some((segment, rest)) => {
            let Some((first, tail)) = link.split_first() else {
                return false;
            };
            let head_matches = match segment {
                Segment::Static(name) => static_matches(first, name),
                _ => !first.is_empty(),
            };
            head_matches && segments_match(tail, rest)
        }
    }
}

fn static_matches(link_segment: &str, name: &str) -> bool {
    let bracket = link_segment.starts_with('[') && link_segment.ends_with(']');
    if bracket || link_segment == WILD.to_string() {
        return true;
    }
    if !link_segment.contains(WILD) {
        return link_segment == name;
    }
    let pieces: Vec<&str> = link_segment.split(WILD).collect();
    let (first, last) = (pieces[0], pieces[pieces.len() - 1]);
    if name.len() < first.len() + last.len() || !name.starts_with(first) || !name.ends_with(last) {
        return false;
    }
    let mut cursor = first.len();
    let limit = name.len() - last.len();
    for piece in &pieces[1..pieces.len() - 1] {
        match name[cursor..limit].find(piece) {
            Some(at) => cursor += at + piece.len(),
            None => return false,
        }
    }
    true
}

/// A flattened string that looks like an absolute app path. A leading
/// wildcard (`${base}/notes`) is kept; `link_matches` reads it as zero or more
/// leading segments.
fn route_shaped(text: &str) -> Option<String> {
    let body = text
        .strip_prefix(WILD)
        .filter(|rest| rest.starts_with('/'))
        .unwrap_or(text);
    (body.starts_with('/') && !body.starts_with("//")).then(|| text.to_string())
}

fn classify(flat: &str) -> NavTarget {
    let text = flat.trim();
    if text.is_empty() {
        return NavTarget::Ignore;
    }
    if !text.chars().any(|ch| ch != WILD && !ch.is_whitespace()) {
        return NavTarget::Unresolved;
    }
    if let Some(route) = route_shaped(text) {
        return NavTarget::Route(route);
    }
    static SCHEME: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9+.\-]*:").expect("valid scheme regex"));
    if text.starts_with("//")
        || text.starts_with('#')
        || text.starts_with('?')
        || SCHEME.is_match(text)
    {
        return NavTarget::Ignore;
    }
    NavTarget::Unresolved
}

static NAV_PATTERNS: LazyLock<Vec<(&'static str, Regex, bool)>> = LazyLock::new(|| {
    let entries: [(&str, &str, bool); 7] = [
        ("href", r"(?:^|[^\w\-.$])href\s*=\s*", false),
        (
            "form_action",
            r"(?s)<form\b(?:[^>]|=>)*?\baction\s*=\s*",
            true,
        ),
        (
            "router_push",
            r"(?:\w*[Rr]outer\w*|useRouter\(\s*\))\s*\??\.\s*(?:push|replace)\s*\(\s*",
            false,
        ),
        (
            "redirect",
            r"(?:^|[^\w.$])(?:permanentRedirect|redirect)\s*\(\s*",
            false,
        ),
        ("location_href", r"\blocation\s*\.\s*href\s*=", false),
        (
            "location_assign",
            r"\blocation\s*\.\s*(?:assign|replace)\s*\(\s*",
            false,
        ),
        ("window_open", r"\bwindow\s*\.\s*open\s*\(\s*", false),
    ];
    entries
        .into_iter()
        .map(|(name, pattern, literal_only)| {
            (
                name,
                Regex::new(pattern).expect("valid navigation regex"),
                literal_only,
            )
        })
        .collect()
});

static ROUTER_ALIAS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"\b(\w+)\s*=\s*useRouter\s*\(",
        r"\}\s*=\s*useRouter\s*\(",
        r"\b(?:redirect|permanentRedirect|useRouter)\s+as\s+(\w+)",
    ]
    .into_iter()
    .map(|pattern| Regex::new(pattern).expect("valid router alias regex"))
    .collect()
});

/// `compilerOptions.paths` keys other than the `@/` alias the scan resolves.
fn tsconfig_path_aliases(project_root: &Path) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for name in ["tsconfig.json", "jsconfig.json"] {
        let Ok(text) = std::fs::read_to_string(project_root.join(name)) else {
            continue;
        };
        let Ok(config) = serde_json::from_str::<serde_json::Value>(&blank_comments(&text)) else {
            continue;
        };
        let Some(paths) = config
            .pointer("/compilerOptions/paths")
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        for key in paths.keys() {
            let prefix = key.trim_end_matches('*');
            if prefix != "@/" && !prefix.is_empty() {
                out.push((prefix.to_string(), prefix.len() == key.len()));
            }
        }
    }
    out
}

static PATHNAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bpathname\s*:\s*").expect("valid pathname regex"));

fn scan_navigations(code: &str, file: &str) -> Vec<Navigation> {
    let mut out = Vec::new();
    let spans = string_spans(code);
    for (syntax, regex, literal_only) in NAV_PATTERNS.iter() {
        let (syntax, literal_only) = (*syntax, *literal_only);
        for found in regex.find_iter(code) {
            let mut start = found.end();
            if syntax == "location_href" && code[start..].starts_with('=') {
                continue;
            }
            while code[start..].starts_with(char::is_whitespace) {
                start += code[start..].chars().next().map_or(1, char::len_utf8);
            }
            let targets = if syntax == "href"
                && replacement_reference_in_string(&code[start..], found.end(), &spans)
            {
                vec![NavTarget::Ignore]
            } else {
                argument_targets(&code[start..], syntax, literal_only)
            };
            let line = line_of(code, found.start());
            out.extend(targets.into_iter().map(|target| Navigation {
                file: file.to_string(),
                line,
                syntax,
                target,
            }));
        }
    }
    out
}

/// Byte ranges (opening quote to closing quote) of every string and template
/// literal in `code`.
fn string_spans(code: &str) -> Vec<(usize, usize)> {
    let bytes = code.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if matches!(bytes[index], b'"' | b'\'' | b'`')
            && let Some((_, end)) = read_string(code, index)
        {
            out.push((index, end));
            index = end;
            continue;
        }
        index += code[index..].chars().next().map_or(1, char::len_utf8);
    }
    out
}

/// A quoted `href` value inside a JS string literal that is only `$1`-style
/// replacement references (`'<a href="$2">'`). Such a value is no navigation.
/// Only used to turn an unresolved reading into `Ignore`; it never makes a
/// route reachable.
fn replacement_reference_in_string(text: &str, offset: usize, spans: &[(usize, usize)]) -> bool {
    static REFERENCES: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?:\$(?:\d+|&))+$").expect("valid reference regex"));
    spans
        .iter()
        .any(|(open, close)| *open < offset && offset < *close)
        && read_string(text, 0).is_some_and(|(flat, _)| {
            let compact: String = flat.split_whitespace().collect();
            REFERENCES.is_match(&compact)
        })
}

fn argument_targets(text: &str, syntax: &str, literal_only: bool) -> Vec<NavTarget> {
    if matches!(syntax, "href" | "form_action")
        && let Some((flat, _)) = read_string(text, 0)
    {
        return vec![classify(&flat)];
    }
    let expression = if text.starts_with('{') && matches!(syntax, "href" | "form_action") {
        match skip_braces(text, 0) {
            Some(end) => &text[1..end - 1],
            None => return vec![NavTarget::Unresolved],
        }
    } else {
        expression_slice(text)
    };
    let expression = expression.trim();
    if literal_only && !expression.starts_with(['"', '\'', '`']) {
        return Vec::new();
    }
    if expression.starts_with('{')
        && let Some(found) = PATHNAME.find(expression)
    {
        let value = expression_slice(&expression[found.end()..]);
        return alternatives(value);
    }
    if expression.starts_with('{') {
        return vec![NavTarget::Unresolved];
    }
    alternatives(expression)
}

fn alternatives(expression: &str) -> Vec<NavTarget> {
    let expression = strip_wrappers(expression);
    if let Some(branches) = split_branches(expression) {
        return branches.into_iter().flat_map(alternatives).collect();
    }
    let mut flat = String::new();
    let mut start = 0;
    let mut cuts = top_level_tokens(expression, "+");
    cuts.push(expression.len());
    for cut in cuts {
        let term = expression[start..cut].trim();
        match read_string(term, 0) {
            Some((parts, end)) if term[end..].trim().is_empty() => flat.push_str(&parts),
            _ => flat.push(WILD),
        }
        start = (cut + 1).min(expression.len());
    }
    vec![classify(&flat)]
}

fn strip_wrappers(expression: &str) -> &str {
    static CAST: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\s+as\s+[\w.]+\s*$").expect("valid cast regex"));
    let mut text = expression.trim();
    loop {
        let before = text;
        if let Some(found) = CAST.find(text) {
            text = text[..found.start()].trim();
        }
        if text.starts_with('(')
            && text.ends_with(')')
            && skip_parens(text).is_some_and(|end| end == text.len())
        {
            text = text[1..text.len() - 1].trim();
        }
        if text == before {
            return text;
        }
    }
}

fn skip_parens(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = 0;
    while index < text.len() {
        let byte = text.as_bytes()[index];
        match byte {
            b'"' | b'\'' | b'`' => {
                if let Some((_, end)) = read_string(text, index) {
                    index = end;
                    continue;
                }
            }
            b'(' => depth += 1,
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn split_branches(expression: &str) -> Option<Vec<&str>> {
    let bytes = expression.as_bytes();
    let question = top_level_tokens(expression, "?").into_iter().find(|&at| {
        let next = bytes.get(at + 1).copied();
        let prev = at.checked_sub(1).and_then(|prev| bytes.get(prev)).copied();
        next != Some(b'.') && next != Some(b'?') && prev != Some(b'?')
    });
    if let Some(at) = question {
        let rest = &expression[at + 1..];
        let colon = *top_level_tokens(rest, ":").first()?;
        return Some(vec![&rest[..colon], &rest[colon + 1..]]);
    }
    if let Some(at) = top_level_tokens(expression, "&&").first() {
        return Some(vec![&expression[at + 2..]]);
    }
    let either = [
        top_level_tokens(expression, "||").first().copied(),
        top_level_tokens(expression, "??").first().copied(),
    ]
    .into_iter()
    .flatten()
    .min()?;
    Some(vec![&expression[..either], &expression[either + 2..]])
}

/// Byte offsets where `token` starts outside brackets and string literals.
fn top_level_tokens(expression: &str, token: &str) -> Vec<usize> {
    let bytes = expression.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' | b'\'' | b'`' => {
                if let Some((_, end)) = read_string(expression, index) {
                    index = end;
                    continue;
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {
                if depth == 0 && expression.as_bytes()[index..].starts_with(token.as_bytes()) {
                    out.push(index);
                    index += token.len();
                    continue;
                }
            }
        }
        index += 1;
    }
    out
}

/// Leading part of `text` that forms one expression: it ends at a top-level
/// `,` `;` or closing bracket, or at a newline that does not continue it.
fn expression_slice(text: &str) -> &str {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' | b'\'' | b'`' => {
                if let Some((_, end)) = read_string(text, index) {
                    index = end;
                    continue;
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                if depth == 0 {
                    return &text[..index];
                }
                depth -= 1;
            }
            b',' | b';' if depth == 0 => return &text[..index],
            b'\n' if depth == 0 && !continues(&text[..index], &text[index + 1..]) => {
                return &text[..index];
            }
            _ => {}
        }
        index += 1;
    }
    text
}

fn continues(before: &str, after: &str) -> bool {
    const OPERATORS: &[char] = &['+', '?', ':', '|', '&'];
    match before.trim_end().chars().last() {
        None => true,
        Some(last) if OPERATORS.contains(&last) => true,
        _ => after.trim_start().starts_with(OPERATORS),
    }
}

/// Reads the string literal starting at `start`. Returns the flattened text
/// (`WILD` for each `${..}`) and the index after the closing quote. Quote
/// strings end at a newline, templates do not.
fn read_string(text: &str, start: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let quote = *bytes.get(start)?;
    if !matches!(quote, b'"' | b'\'' | b'`') {
        return None;
    }
    let mut out = String::new();
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => {
                let next = text[index + 1..].chars().next()?;
                out.push(next);
                index += 1 + next.len_utf8();
            }
            byte if byte == quote => return Some((out, index + 1)),
            b'\n' if quote != b'`' => return None,
            b'$' if quote == b'`' && bytes.get(index + 1) == Some(&b'{') => {
                out.push(WILD);
                index = skip_braces(text, index + 1)?;
            }
            _ => {
                let ch = text[index..].chars().next()?;
                out.push(ch);
                index += ch.len_utf8();
            }
        }
    }
    None
}

/// Index after the `}` that matches the `{` at `open`.
fn skip_braces(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'"' | b'\'' | b'`' => {
                if let Some((_, end)) = read_string(text, index) {
                    index = end;
                    continue;
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// Replaces comments with spaces (newlines kept) so offsets and lines hold.
fn blank_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if matches!(byte, b'"' | b'\'' | b'`')
            && let Some((_, end)) = read_string(text, index)
        {
            out.push_str(&text[index..end]);
            index = end;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            while index < bytes.len() && bytes[index] != b'\n' {
                out.push(' ');
                index += 1;
            }
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            let end = text[index + 2..]
                .find("*/")
                .map_or(bytes.len(), |at| index + 2 + at + 2);
            for &skipped in &bytes[index..end] {
                out.push(if skipped == b'\n' { '\n' } else { ' ' });
            }
            index = end;
            continue;
        }
        let ch = text[index..].chars().next().unwrap_or(' ');
        out.push(ch);
        index += ch.len_utf8();
    }
    out
}

/// Every string and template literal (flattened) with its byte offset.
fn string_literals(code: &str) -> Vec<(usize, String)> {
    let bytes = code.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if matches!(bytes[index], b'"' | b'\'' | b'`')
            && let Some((text, end)) = read_string(code, index)
        {
            out.push((index, text));
            index = end;
            continue;
        }
        index += code[index..].chars().next().map_or(1, char::len_utf8);
    }
    out
}

fn line_of(text: &str, offset: usize) -> usize {
    text.as_bytes()[..offset.min(text.len())]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

#[cfg(test)]
#[path = "route_reachability_tests.rs"]
mod tests;
