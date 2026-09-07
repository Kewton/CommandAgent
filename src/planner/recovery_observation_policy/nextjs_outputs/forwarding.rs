//! One local function boundary, with module-constant actual arguments only.
use super::*;

struct Helper<'a> {
    name: &'a str,
    name_position: usize,
    parameters: Vec<(usize, &'a str)>,
    body_start: usize,
    body_end: usize,
}

impl Source {
    pub(super) fn forwarded_writer_paths(
        &self,
        bindings: &BTreeMap<String, Vec<Binding>>,
    ) -> Vec<String> {
        let mut paths = Vec::new();
        for declaration in 0..self.tokens.len() {
            let Some(helper) = self.writer_helper(declaration) else {
                continue;
            };
            // Every other reference must be an unqualified call, not an alias,
            // assignment, shadowing declaration, property, or opaque expression.
            if self.opaque_names.contains(helper.name)
                || self.tokens.iter().enumerate().any(|(i, _)| {
                    self.identifier(i) == Some(helper.name)
                        && i != helper.name_position
                        && (i > 0 && (self.is(i - 1, ".") || self.is(i - 1, "function"))
                            || !self.is(i + 1, "("))
                })
            {
                continue;
            }
            for (argument_index, (parameter_position, parameter)) in
                helper.parameters.iter().enumerate()
            {
                if self.opaque_names.contains(*parameter) {
                    continue;
                }
                let mut writer_arguments = Vec::new();
                for i in helper.body_start + 1..helper.body_end {
                    if i > 0 && self.is(i - 1, ".") {
                        continue;
                    }
                    if let Some((Value::Writer, end)) = self.expression(i, bindings, 0)
                        && self.is(end, "(")
                        && self.identifier(end + 1) == Some(*parameter)
                        && self.is(end + 2, ",")
                        && self.scopes[i] == self.scopes[helper.body_start + 1]
                    {
                        writer_arguments.push(end + 1);
                    }
                }
                // A parameter is a pure forwarding slot only when every use in
                // its function is the complete first argument of a known writer.
                if writer_arguments.is_empty()
                    || (helper.body_start + 1..helper.body_end).any(|i| {
                        self.identifier(i) == Some(*parameter) && !writer_arguments.contains(&i)
                    })
                    || helper
                        .parameters
                        .iter()
                        .any(|(pos, name)| name == parameter && pos != parameter_position)
                {
                    continue;
                }
                for call in 0..self.tokens.len() {
                    if call == helper.name_position
                        || self.identifier(call) != Some(helper.name)
                        || !self.is(call + 1, "(")
                        || (helper.body_start..=helper.body_end).contains(&call)
                    {
                        continue;
                    }
                    let Some(arguments) = self.call_arguments(call + 1) else {
                        continue;
                    };
                    let Some(&(start, end)) = arguments.get(argument_index) else {
                        continue;
                    };
                    // A declaration/method with this spelling is not a caller.
                    let close = arguments.last().map(|(_, end)| *end).unwrap_or(call + 2);
                    if self.is(close + 1, "{")
                        || self.is(close + 1, ":")
                        || self.sequence(close + 1, &["=", ">"])
                    {
                        continue;
                    }
                    let Some(name) = self.identifier(start) else {
                        continue;
                    };
                    if end != start + 1
                        || !bindings.get(name).is_some_and(|values| {
                            values.len() == 1
                                && values[0].scope == 0
                                && values[0].expression.is_some()
                        })
                    {
                        continue;
                    }
                    if let Some((Value::Path(path), next)) = self.expression(start, bindings, 0)
                        && next == end
                    {
                        paths.push(path);
                    }
                }
            }
        }
        paths
    }

    fn writer_helper(&self, declaration: usize) -> Option<Helper<'_>> {
        if !self.is(declaration, "function") || self.scopes[declaration] != 0 {
            return None;
        }
        let mut before = declaration;
        for prefix in ["async", "export"] {
            if before > 0 && self.is(before - 1, prefix) {
                before -= 1;
            }
        }
        if before > 0 && !self.is(before - 1, ";") && !self.is(before - 1, "}") {
            return None;
        }
        let name_position = declaration + 1;
        let name = self.identifier(name_position)?;
        let mut open = name_position + 1;
        // The real E3 helper declares a single generic type parameter.
        if self.is(open, "<") && self.identifier(open + 1).is_some() && self.is(open + 2, ">") {
            open += 3;
        }
        if !self.is(open, "(") {
            return None;
        }
        let arguments = self.call_arguments(open)?;
        let mut parameters = Vec::new();
        for &(start, end) in &arguments {
            let parameter = self.identifier(start)?;
            if end != start + 1
                && (!self.is(start + 1, ":")
                    || !(start + 2..end).all(|i| self.simple_type_token(i)))
            {
                return None;
            }
            parameters.push((start, parameter));
        }
        let mut body_start = arguments.last()?.1 + 1;
        if self.is(body_start, ":") {
            body_start += 1;
            while self.simple_type_token(body_start) {
                body_start += 1;
            }
        }
        if !self.is(body_start, "{") {
            return None;
        }
        let body_end = (body_start + 1..self.tokens.len())
            .find(|&i| self.is(i, "}") && self.parents[self.scopes[i]] == Some(0))?;
        Some(Helper {
            name,
            name_position,
            parameters,
            body_start,
            body_end,
        })
    }

    fn simple_type_token(&self, i: usize) -> bool {
        self.identifier(i).is_some() || ["[", "]", "<", ">", "|"].iter().any(|s| self.is(i, s))
    }

    fn call_arguments(&self, open: usize) -> Option<Vec<(usize, usize)>> {
        let mut stack = vec![")"];
        let mut start = open + 1;
        let mut arguments = Vec::new();
        for i in open + 1..self.tokens.len() {
            let closing = if self.is(i, "(") {
                Some(")")
            } else if self.is(i, "[") {
                Some("]")
            } else if self.is(i, "{") {
                Some("}")
            } else {
                None
            };
            if let Some(closing) = closing {
                stack.push(closing);
            } else if [")", "]", "}"].iter().any(|s| self.is(i, s)) {
                if !self.is(i, stack.pop()?) {
                    return None;
                }
                if stack.is_empty() {
                    if i > start {
                        arguments.push((start, i));
                    }
                    return Some(arguments);
                }
            } else if self.is(i, ",") && stack.len() == 1 {
                if i == start {
                    return None;
                }
                arguments.push((start, i));
                start = i + 1;
            }
        }
        None
    }
}
