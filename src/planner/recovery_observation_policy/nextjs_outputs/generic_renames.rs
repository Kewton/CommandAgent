//! Resolve a rename parameter backwards through bounded local call sites.
//! All references and all actual arguments must be proved; a helper's existence
//! or export alone never authorizes a path. Opaque calls and escapes fail closed.
use super::{forwarding::Helper, *};

pub(super) fn foreign_references(
    workspace: &Path,
    own: &Path,
    candidates: &BTreeSet<PathBuf>,
) -> Option<BTreeSet<String>> {
    // Scan a conservative superset of identifiers, including strings/comments
    // and JSX that this small parser cannot otherwise analyze. Namespace and
    // dynamic imports/re-exports can hide a helper name behind computed access.
    static OPAQUE_IMPORT: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?s)\b(?:import|export)\b(?:\s|/\*.*?\*/|//[^\n]*\n)*(?:\*|\()")
            .unwrap()
    });
    let mut names = BTreeSet::new();
    for path in candidates.iter().filter(|p| p.as_path() != own) {
        if !confined_path(workspace, path) {
            return None;
        }
        let text = std::fs::read_to_string(workspace.join(path)).ok()?;
        // Unicode escapes can spell imported/local identifiers without their
        // raw names. Do not decode or trust a partial spelling, even inside a
        // comment/string; this removes grants only and also covers JSX modules.
        if OPAQUE_IMPORT.is_match(&text) || text.contains(r"\u") {
            return None;
        }
        names.extend(
            text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '$')
                .filter(|word| !word.is_empty())
                .map(str::to_owned),
        );
    }
    if ["require", "eval", "Function", "constructor"]
        .iter()
        .any(|name| names.contains(*name))
    {
        return None;
    }
    Some(names)
}

struct Proof<'a> {
    source: &'a Source,
    bindings: &'a BTreeMap<String, Vec<Binding>>,
    helpers: Vec<Helper<'a>>,
    foreign_names: &'a BTreeSet<String>,
}

impl Source {
    pub(super) fn generic_rename_paths(
        &self,
        bindings: &BTreeMap<String, Vec<Binding>>,
        foreign_names: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        let proof = Proof {
            source: self,
            bindings,
            helpers: (0..self.tokens.len())
                .filter_map(|i| self.writer_helper(i))
                .collect(),
            foreign_names,
        };
        let mut paths = BTreeSet::new();
        for i in 0..self.tokens.len() {
            if i > 0 && self.is(i - 1, ".") {
                continue;
            }
            let Some((Value::Renamer, open)) = self.expression(i, bindings, 0) else {
                continue;
            };
            let Some(args) = self
                .call_arguments(open)
                .filter(|a| self.is(open, "(") && a.len() == 2)
            else {
                continue;
            };
            let (start, end) = args[1];
            let Some((helper, slot)) = proof.parameter_at(start) else {
                continue;
            };
            if end == start + 1
                && let Some(outputs) = proof.origins(helper, slot, &mut BTreeSet::new())
            {
                paths.extend(outputs);
            }
        }
        paths
    }
}

impl Proof<'_> {
    fn parameter_at(&self, position: usize) -> Option<(usize, usize)> {
        self.helpers.iter().enumerate().find_map(|(index, helper)| {
            (helper.body_start < position && position < helper.body_end)
                .then(|| {
                    helper
                        .parameters
                        .iter()
                        .position(|(_, name)| self.source.identifier(position) == Some(*name))
                        .map(|slot| (index, slot))
                })
                .flatten()
        })
    }

    fn call(&self, position: usize) -> Option<Vec<(usize, usize)>> {
        let s = self.source;
        let mut open = position + 1;
        // A single explicit type argument is erased by TypeScript, not a path.
        if s.is(open, "<") && s.identifier(open + 1).is_some() && s.is(open + 2, ">") {
            open += 3;
        }
        if !s.is(open, "(") || position > 0 && s.is(position - 1, ".") {
            return None;
        }
        let args = s.call_arguments(open)?;
        let close = args.last()?.1;
        if s.is(close + 1, "{") || s.is(close + 1, ":") || s.sequence(close + 1, &["=", ">"]) {
            return None;
        }
        Some(args)
    }

    fn origins(
        &self,
        index: usize,
        slot: usize,
        visiting: &mut BTreeSet<(usize, usize)>,
    ) -> Option<BTreeSet<String>> {
        if visiting.len() >= 8 || !visiting.insert((index, slot)) {
            return None;
        }
        let result = self.resolve(index, slot, visiting);
        visiting.remove(&(index, slot));
        result
    }

    fn resolve(
        &self,
        index: usize,
        slot: usize,
        visiting: &mut BTreeSet<(usize, usize)>,
    ) -> Option<BTreeSet<String>> {
        let s = self.source;
        let helper = &self.helpers[index];
        if self.foreign_names.contains(helper.name)
            || s.opaque_names.contains(helper.name)
            || !self.pure_slot(helper, slot)
        {
            return None;
        }
        let mut outputs = BTreeSet::new();
        for position in 0..s.tokens.len() {
            if s.identifier(position) != Some(helper.name) || position == helper.name_position {
                continue;
            }
            let args = self.call(position)?;
            if args.len() != helper.parameters.len() {
                return None;
            }
            let &(start, end) = args.get(slot)?;
            if end != start + 1 {
                return None;
            }
            let name = s.identifier(start)?;
            if let Some((caller, parameter)) = self.parameter_at(start) {
                outputs.extend(self.origins(caller, parameter, visiting)?);
            } else {
                // Only an immutable module constant; no literal/dynamic call-site
                // argument, local shadow, conditional or guessed fallback value.
                let definitions = self.bindings.get(name)?;
                if definitions.len() != 1 || definitions[0].scope != 0 {
                    return None;
                }
                let (Value::Path(path), next) = s.expression(start, self.bindings, 0)? else {
                    return None;
                };
                if next != end {
                    return None;
                }
                outputs.insert(path);
            }
        }
        (!outputs.is_empty()).then_some(outputs)
    }

    fn pure_slot(&self, helper: &Helper<'_>, slot: usize) -> bool {
        let s = self.source;
        let (_, parameter) = helper.parameters[slot];
        if s.opaque_names.contains(parameter)
            || helper
                .parameters
                .iter()
                .filter(|(_, n)| *n == parameter)
                .count()
                != 1
        {
            return false;
        }
        let mut reads = BTreeSet::new();
        for i in helper.body_start + 1..helper.body_end {
            if i > 0 && s.is(i - 1, ".") {
                continue;
            }
            // Local forwarding uses the full parameter as an actual argument.
            if self.helpers.iter().any(|h| s.identifier(i) == Some(h.name))
                && let Some(args) = self.call(i)
            {
                for (start, end) in args {
                    if end == start + 1 && s.identifier(start) == Some(parameter) {
                        reads.insert(start);
                    }
                }
            }
            if let Some((Value::Renamer, open)) = s.expression(i, self.bindings, 0)
                && s.is(open, "(")
                && let Some(args) = s.call_arguments(open).filter(|a| a.len() == 2)
            {
                let (start, end) = args[1];
                if end == start + 1 && s.identifier(start) == Some(parameter) {
                    reads.insert(start);
                }
            }
            // These builtin string/path reads cannot rebind a parameter. Require
            // the original imported receiver, with normal ambiguity checks.
            let Some(receiver) = s.identifier(i) else {
                continue;
            };
            let Some(defs) = self.bindings.get(receiver) else {
                continue;
            };
            if s.unsafe_names.contains(receiver) || defs.len() != 1 || defs[0].scope != 0 {
                continue;
            }
            let method_ok = match defs[0].value {
                Some(Value::PathModule) => s.is(i + 2, "dirname") || s.is(i + 2, "basename"),
                Some(Value::Fs | Value::Promises) => s.is(i + 2, "readFile"),
                _ => false,
            };
            if method_ok
                && s.is(i + 1, ".")
                && s.is(i + 3, "(")
                && let Some(args) = s.call_arguments(i + 3)
                && let Some(&(start, end)) = args.first()
                && end == start + 1
                && s.identifier(start) == Some(parameter)
            {
                reads.insert(start);
            }
        }
        !reads.is_empty()
            && (helper.body_start + 1..helper.body_end)
                .all(|i| s.identifier(i) != Some(parameter) || reads.contains(&i))
    }
}
