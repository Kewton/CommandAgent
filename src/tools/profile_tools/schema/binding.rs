//! The single immutable revision of a tool's inputs, and the schema derived from
//! it. The first admission in a process adopts them, before the first provider
//! call. Every later admission, receipt, and execution compares with what was
//! adopted and refuses a difference: nothing here adopts a second time, and a value
//! a model sent or a history holds is compared, never taken as the expectation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::super::descriptor::read_without_following;
use super::super::{ProfileToolRefusal, RefusalKind};
use super::admission::{self, CompiledSchema};
use super::{Limits, SchemaDeclaration};

const MAX_INPUTS: usize = 64;
const MAX_INPUT_BYTES: u64 = 1 << 20;
const MAX_TOTAL_INPUT_BYTES: usize = 8 << 20;

/// One input file the host adopted, with its workspace-relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaInput {
    pub path: String,
    pub bytes: Vec<u8>,
}

/// What an operator's compiler receives: only what the host adopted.
#[derive(Debug)]
pub struct SchemaCompileInput<'a> {
    pub static_root: &'a Value,
    pub input_revision: &'a str,
    pub inputs: &'a [SchemaInput],
}

/// An operator's deterministic rule from adopted input bytes to a schema. The host
/// admits its result like any other schema and pins the digest. It must not call a
/// model or read anything else: running it again on the same bytes must give the
/// same schema, or the run is refused.
pub trait ProfileToolSchemaCompiler: fmt::Debug + Send + Sync {
    fn compile(&self, input: &SchemaCompileInput<'_>) -> anyhow::Result<Value>;
}

type CompilerTable = BTreeMap<String, Arc<dyn ProfileToolSchemaCompiler>>;

static COMPILERS: Mutex<CompilerTable> = Mutex::new(BTreeMap::new());

/// Make a compiler available under an ID. This is the only way to add one; the
/// `commandagent` binary registers none.
pub fn register_schema_compiler(id: &str, compiler: Arc<dyn ProfileToolSchemaCompiler>) {
    COMPILERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(id.to_string(), compiler);
}

/// What one tool adopted, for one workspace and profile.
#[derive(Debug)]
pub(crate) struct Adoption {
    declaration: SchemaDeclaration,
    tool_id: String,
    tool_version: String,
    descriptor_sha256: String,
    workspace: PathBuf,
    input_revision: String,
    inputs: Vec<SchemaInput>,
    schema: CompiledSchema,
}

type AdoptionKey = (PathBuf, String, String);

static ADOPTIONS: Mutex<BTreeMap<AdoptionKey, Arc<Adoption>>> = Mutex::new(BTreeMap::new());

fn adoptions() -> std::sync::MutexGuard<'static, BTreeMap<AdoptionKey, Arc<Adoption>>> {
    ADOPTIONS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The identity of one tool as admitted.
pub(crate) struct ToolPins<'a> {
    pub(crate) tool_id: &'a str,
    pub(crate) version: &'a str,
    pub(crate) descriptor_sha256: &'a str,
    pub(crate) max_input_bytes: usize,
}

/// Adopt on first sight, compare ever after. The adoption lasts as long as the
/// process, so a repair, a later session, and a `/resume` in the same process meet
/// the same revision. Key: workspace, profile, tool.
pub(crate) fn adopt(
    schema: &Value,
    pins: &ToolPins<'_>,
    workspace: &Path,
    profile: &str,
) -> Result<Arc<Adoption>, ProfileToolRefusal> {
    let key = (
        workspace.to_path_buf(),
        profile.to_string(),
        pins.tool_id.to_string(),
    );
    let existing = adoptions().get(&key).cloned();
    if let Some(existing) = existing {
        existing.verify_admission(pins)?;
        return Ok(existing);
    }
    let declaration = SchemaDeclaration::decode(schema, pins.tool_id)?;
    let limits = declaration.effective_limits(pins.tool_id, pins.max_input_bytes)?;
    let (inputs, input_revision) = read_inputs(&declaration, workspace, pins.tool_id)?;
    let schema = effective_schema(&declaration, &input_revision, &inputs, limits, pins.tool_id)?;
    let adoption = Arc::new(Adoption {
        declaration,
        tool_id: pins.tool_id.to_string(),
        tool_version: pins.version.to_string(),
        descriptor_sha256: pins.descriptor_sha256.to_string(),
        workspace: workspace.to_path_buf(),
        input_revision,
        inputs,
        schema,
    });
    let held = adoptions().entry(key).or_insert(adoption).clone();
    held.verify_admission(pins)?;
    Ok(held)
}

impl Adoption {
    pub(crate) fn input_revision(&self) -> &str {
        &self.input_revision
    }

    pub(crate) fn schema(&self) -> &CompiledSchema {
        &self.schema
    }

    fn verify_admission(&self, pins: &ToolPins<'_>) -> Result<(), ProfileToolRefusal> {
        if self.descriptor_sha256 != pins.descriptor_sha256 || self.tool_version != pins.version {
            return Err(ProfileToolRefusal::new(
                RefusalKind::SchemaMismatch,
                format!(
                    "tool `{}` descriptor differs from the one its schema was adopted with",
                    self.tool_id
                ),
            ));
        }
        self.verify("admission")
    }

    /// The bytes on disk must hold the adopted revision, and the compiler must still
    /// give the adopted schema from the adopted bytes.
    pub(crate) fn verify(&self, stage: &str) -> Result<(), ProfileToolRefusal> {
        let tool = &self.tool_id;
        let revision = read_inputs(&self.declaration, &self.workspace, tool)
            .map(|(_, revision)| revision)
            .map_err(|refusal| {
                ProfileToolRefusal::new(
                    RefusalKind::InputRevisionMismatch,
                    format!("stage={stage} tool `{tool}`: {}", refusal.reason),
                )
            })?;
        if revision != self.input_revision {
            return Err(ProfileToolRefusal::new(
                RefusalKind::InputRevisionMismatch,
                format!("stage={stage} tool `{tool}` inputs changed since they were adopted"),
            ));
        }
        let matches = effective_schema(
            &self.declaration,
            &self.input_revision,
            &self.inputs,
            self.schema.limits(),
            tool,
        )
        .is_ok_and(|schema| schema.sha256() == self.schema.sha256());
        if !matches {
            return Err(ProfileToolRefusal::new(
                RefusalKind::SchemaMismatch,
                format!("stage={stage} tool `{tool}` schema differs from the adopted hash"),
            ));
        }
        Ok(())
    }
}

/// The revision is a digest of the declared inputs, in declared order: for each,
/// its path, a NUL, its length in decimal, a NUL, and its bytes.
fn read_inputs(
    declaration: &SchemaDeclaration,
    workspace: &Path,
    tool: &str,
) -> Result<(Vec<SchemaInput>, String), ProfileToolRefusal> {
    let unavailable = |reason: String| {
        ProfileToolRefusal::new(
            RefusalKind::InputRevisionUnavailable,
            format!("tool `{tool}` {reason}"),
        )
    };
    if declaration.inputs.len() > MAX_INPUTS {
        return Err(unavailable(format!(
            "declares more than {MAX_INPUTS} inputs"
        )));
    }
    let mut seen = BTreeSet::new();
    let mut inputs = Vec::with_capacity(declaration.inputs.len());
    let mut total = 0usize;
    let mut digest = Sha256::new();
    for path in &declaration.inputs {
        if crate::tools::path_guard::validate_workspace_relative(path).is_err()
            || !seen.insert(path.as_str())
        {
            return Err(unavailable(
                "declares an input path that is not one distinct workspace-relative path"
                    .to_string(),
            ));
        }
        let bytes = read_without_following(workspace, Path::new(path), MAX_INPUT_BYTES)
            .map_err(|reason| unavailable(format!("input {reason}")))?;
        total += bytes.len();
        if total > MAX_TOTAL_INPUT_BYTES {
            return Err(unavailable(format!(
                "inputs exceed {MAX_TOTAL_INPUT_BYTES} bytes"
            )));
        }
        digest.update(path.as_bytes());
        digest.update([0]);
        digest.update(bytes.len().to_string().as_bytes());
        digest.update([0]);
        digest.update(&bytes);
        inputs.push(SchemaInput {
            path: path.clone(),
            bytes,
        });
    }
    Ok((inputs, format!("{:x}", digest.finalize())))
}

fn effective_schema(
    declaration: &SchemaDeclaration,
    revision: &str,
    inputs: &[SchemaInput],
    limits: Limits,
    tool: &str,
) -> Result<CompiledSchema, ProfileToolRefusal> {
    let root = match &declaration.compiler {
        Some(id) => run_compiler(id, declaration, revision, inputs, tool)?,
        None => declaration.root.clone(),
    };
    let root = bind_revision_argument(root, declaration, revision, tool)?;
    admission::compile(&root, limits).map_err(|refusal| {
        ProfileToolRefusal::new(refusal.kind, format!("tool `{tool}`: {}", refusal.reason))
    })
}

fn run_compiler(
    id: &str,
    declaration: &SchemaDeclaration,
    revision: &str,
    inputs: &[SchemaInput],
    tool: &str,
) -> Result<Value, ProfileToolRefusal> {
    let compiler = COMPILERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(id)
        .cloned()
        .ok_or_else(|| {
            ProfileToolRefusal::new(
                RefusalKind::Unavailable,
                format!("tool `{tool}` schema compiler `{id}` is not available in this process"),
            )
        })?;
    compiler
        .compile(&SchemaCompileInput {
            static_root: &declaration.root,
            input_revision: revision,
            inputs,
        })
        .map_err(|_| {
            ProfileToolRefusal::new(
                RefusalKind::SchemaUnsupported,
                format!("tool `{tool}` schema compiler `{id}` failed"),
            )
        })
}

/// `revision_argument` names a required top-level string. The host narrows it to
/// the adopted revision, so a call echoing an old revision fails the same validator
/// as any other enum mismatch. The expected value comes from here alone.
fn bind_revision_argument(
    mut root: Value,
    declaration: &SchemaDeclaration,
    revision: &str,
    tool: &str,
) -> Result<Value, ProfileToolRefusal> {
    let Some(argument) = &declaration.revision_argument else {
        return Ok(root);
    };
    let required = root
        .get("required")
        .and_then(Value::as_array)
        .is_some_and(|required| required.iter().any(|name| name == argument.as_str()));
    let slot = root
        .get_mut("properties")
        .and_then(|properties| properties.get_mut(argument))
        .filter(|property| **property == json!({"type": "string"}));
    match slot {
        Some(property) if required => {
            property["enum"] = json!([revision]);
            Ok(root)
        }
        _ => Err(ProfileToolRefusal::new(
            RefusalKind::SchemaUnsupported,
            format!(
                "tool `{tool}` revision_argument must name a required property that is exactly `type: string`"
            ),
        )),
    }
}
