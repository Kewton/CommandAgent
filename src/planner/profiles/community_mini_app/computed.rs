use std::collections::{BTreeMap, BTreeSet};

use serde_yaml::Value;

pub(super) const ENTRY_FIELDS: &[&str] = &["name", "entity", "expression", "type"];
pub(super) const ALLOWED_FUNCTIONS: &[&str] = &["min", "max", "len"];
const MAX_NODES: usize = 64;

type ComputedKey = (String, String);

fn required_string<'a>(
    item: &'a serde_yaml::Mapping,
    field: &str,
    error: &str,
) -> Result<&'a str, String> {
    item.get(Value::String(field.to_string()))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| error.to_string())
}

fn expression_names(expression: &str) -> Result<Vec<&str>, String> {
    let tokens = expression
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.len() > MAX_NODES {
        return Err("community_computed_ast_limit".to_string());
    }
    Ok(tokens
        .into_iter()
        .filter(|token| {
            token
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_alphabetic())
        })
        .collect())
}

pub(super) fn validate_graph(
    computed: &[Value],
    entities: &BTreeMap<String, BTreeSet<String>>,
    allowed_types: &[&str],
) -> Result<Vec<String>, String> {
    let mut definitions = BTreeMap::<ComputedKey, String>::new();
    let mut owners_by_name = BTreeMap::<String, BTreeSet<String>>::new();
    for value in computed {
        let item = value
            .as_mapping()
            .ok_or_else(|| "community_computed_invalid".to_string())?;
        let item_fields = item
            .keys()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        if item_fields != ENTRY_FIELDS.iter().copied().collect::<BTreeSet<_>>() {
            return Err("community_computed_vocabulary_mismatch".to_string());
        }
        let name = required_string(item, "name", "community_computed_name_missing")?;
        let entity = required_string(item, "entity", "community_computed_entity_missing")?;
        let expression =
            required_string(item, "expression", "community_computed_expression_missing")?;
        let result_type = required_string(item, "type", "community_computed_type_missing")?;
        let entity_fields = entities
            .get(entity)
            .ok_or_else(|| format!("community_computed_entity_unknown:{entity}"))?;
        if !allowed_types.contains(&result_type) {
            return Err(format!("community_computed_type_invalid:{result_type}"));
        }
        if entity_fields.contains(name) {
            return Err(format!(
                "community_computed_name_conflicts_field:{entity}.{name}"
            ));
        }
        let key = (entity.to_string(), name.to_string());
        if definitions.insert(key, expression.to_string()).is_some() {
            return Err(format!("community_computed_duplicate:{entity}.{name}"));
        }
        owners_by_name
            .entry(name.to_string())
            .or_default()
            .insert(entity.to_string());
    }

    let mut field_owners = BTreeMap::<String, BTreeSet<String>>::new();
    for (entity, fields) in entities {
        for field in fields {
            field_owners
                .entry(field.clone())
                .or_default()
                .insert(entity.clone());
        }
    }

    let mut dependencies = definitions
        .keys()
        .cloned()
        .map(|key| (key, BTreeSet::<ComputedKey>::new()))
        .collect::<BTreeMap<_, _>>();
    for ((entity, name), expression) in &definitions {
        let entity_fields = &entities[entity];
        for token in expression_names(expression)? {
            if matches!(token, "true" | "false") || ALLOWED_FUNCTIONS.contains(&token) {
                continue;
            }
            if matches!(token, "eval" | "fetch" | "process" | "import") {
                return Err(format!("community_computed_forbidden:{token}"));
            }
            if entity_fields.contains(token) {
                continue;
            }
            let dependency = (entity.clone(), token.to_string());
            if definitions.contains_key(&dependency) {
                dependencies
                    .get_mut(&(entity.clone(), name.clone()))
                    .expect("computed node is registered")
                    .insert(dependency);
                continue;
            }
            let cross_entity_computed = owners_by_name
                .get(token)
                .is_some_and(|owners| owners.iter().any(|owner| owner != entity));
            let cross_entity_field = field_owners
                .get(token)
                .is_some_and(|owners| owners.iter().any(|owner| owner != entity));
            if cross_entity_computed || cross_entity_field {
                return Err(format!("community_computed_cross_entity_reference:{token}"));
            }
            return Err(format!("community_computed_unregistered:{token}"));
        }
    }

    let mut indegree = dependencies
        .iter()
        .map(|(node, requires)| (node.clone(), requires.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<ComputedKey, BTreeSet<ComputedKey>>::new();
    for (node, requires) in &dependencies {
        for required in requires {
            dependents
                .entry(required.clone())
                .or_default()
                .insert(node.clone());
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(node, _)| node.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(definitions.len());
    while let Some(node) = ready.pop_first() {
        order.push(format!("{}.{}", node.0, node.1));
        if let Some(nodes) = dependents.get(&node) {
            for dependent in nodes {
                let count = indegree
                    .get_mut(dependent)
                    .expect("dependent computed node is registered");
                *count -= 1;
                if *count == 0 {
                    ready.insert(dependent.clone());
                }
            }
        }
    }
    if order.len() != definitions.len() {
        let cycle = indegree
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .map(|((entity, name), _)| format!("{entity}.{name}"))
            .collect::<Vec<_>>()
            .join(",");
        return Err(format!("community_computed_cycle:{cycle}"));
    }
    Ok(order)
}
