//! A rename changes its destination, not its first (source) argument.
//! Only complete, already-provable destinations receive the normal JSON-output
//! filters. Opaque helper parameters and dynamic expressions grant no authority.
use super::*;

impl Source {
    pub(super) fn renamed_paths(&self, bindings: &BTreeMap<String, Vec<Binding>>) -> Vec<String> {
        let mut paths = Vec::new();
        for i in 0..self.tokens.len() {
            if i > 0 && self.is(i - 1, ".") {
                continue;
            }
            let Some((Value::Renamer, end)) = self.expression(i, bindings, 0) else {
                continue;
            };
            if !self.is(end, "(") {
                continue;
            }
            let Some(arguments) = self.call_arguments(end) else {
                continue;
            };
            if !(2..=3).contains(&arguments.len()) {
                continue;
            }
            let (start, end) = arguments[1];
            if let Some((Value::Path(path), next)) = self.expression(start, bindings, 0)
                && next == end
            {
                paths.push(path);
            }
        }
        paths
    }
}

#[cfg(test)]
mod tests;
