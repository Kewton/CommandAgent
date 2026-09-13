//! Whole-program structural predicates, never business Test evidence.
use super::import_check::{local_path, source};

pub(super) fn recognizes(command: &str) -> bool {
    if crate::planner::profiles::nextjs::recovery_authority::is_package_check(command) {
        return true;
    }
    let Some(source) = source(command) else {
        return false;
    };
    route_paths(&source).is_some() || page_path(&source).is_some()
}

fn route_paths(source: &str) -> Option<Vec<&str>> {
    let list = source
        .strip_prefix("const fs=require('fs');[")?
        .strip_suffix("].forEach(f=>{if(!fs.existsSync(f))throw new Error('missing '+f)})")?;
    let paths = list
        .split(',')
        .map(|literal| {
            let path = literal.strip_prefix('\'')?.strip_suffix('\'')?;
            local_path(path).then_some(path)
        })
        .collect::<Option<Vec<_>>>()?;
    (!paths.is_empty()).then_some(paths)
}

fn page_path(source: &str) -> Option<&str> {
    let rest = source.strip_prefix("const fs=require('fs');const p=fs.readFileSync('")?;
    let (path, rest) = rest.split_once('\'')?;
    (local_path(path)
        && rest
            == concat!(
                ",'utf8');if(!p.includes('data-anvil-action'))throw new Error('missing anvil');",
                "if(!p.includes('data-anvil-state'))throw new Error('missing state');",
                "if(!p.includes('use client'))throw new Error('missing use client')"
            ))
    .then_some(path)
}
