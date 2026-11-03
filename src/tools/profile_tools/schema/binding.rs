//! The single immutable revision of a tool's inputs, and the schema derived from
//! it. The first admission in a process adopts them, before the first provider
//! call. Every later admission, receipt, and execution compares with what was
//! adopted and refuses a difference: nothing here adopts a second time, and a value
//! a model sent or a history holds is compared, never taken as the expectation.
//!
//! The one exception is an input a declared stage's output feeds (G4). It holds nothing until
//! that output's commit receipt comes, and takes up a new version only from a receipt whose
//! digest the bytes on disk match ([`Adoption::readopt`]). Every other input stays as strict as
//! above, and a tool with no such input takes none of these paths.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::super::commit::CommitReceipt;
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

/// The input bytes a process receives, and the revision a request is bound to. An upstream
/// input with no receipt yet is not among them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdoptedInputs {
    pub(crate) revision: String,
    pub(crate) inputs: Vec<SchemaInput>,
}

/// What one tool adopted, for one workspace and profile.
#[derive(Debug)]
pub(crate) struct Adoption {
    declaration: SchemaDeclaration,
    tool_id: String,
    tool_version: String,
    descriptor_sha256: String,
    workspace: PathBuf,
    /// The inputs a stage's output feeds. Empty for a tool outside a stage set.
    upstream: BTreeSet<String>,
    /// The revision with no upstream bytes in it: what the bytes on disk are compared with.
    fixed_revision: String,
    current: RwLock<Arc<AdoptedInputs>>,
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
#[cfg(test)]
pub(crate) fn adopt(
    schema: &Value,
    pins: &ToolPins<'_>,
    workspace: &Path,
    profile: &str,
) -> Result<Arc<Adoption>, ProfileToolRefusal> {
    adopt_with_upstream(schema, pins, workspace, profile, &BTreeSet::new())
}

/// The same for a tool in a stage set, with the inputs a stage's output feeds.
pub(crate) fn adopt_with_upstream(
    schema: &Value,
    pins: &ToolPins<'_>,
    workspace: &Path,
    profile: &str,
    upstream: &BTreeSet<String>,
) -> Result<Arc<Adoption>, ProfileToolRefusal> {
    let key = (
        workspace.to_path_buf(),
        profile.to_string(),
        pins.tool_id.to_string(),
    );
    let existing = adoptions().get(&key).cloned();
    if let Some(existing) = existing {
        existing.verify_admission(pins, upstream)?;
        return Ok(existing);
    }
    let declaration = SchemaDeclaration::decode(schema, pins.tool_id)?;
    // W1: what upstream content cannot do, whatever else admitted the declaration.
    if !upstream.is_empty()
        && (declaration.compiler.is_some() || declaration.revision_argument.is_some())
    {
        return Err(ProfileToolRefusal::new(
            RefusalKind::SchemaUnsupported,
            format!(
                "tool `{}` reads a stage's output, so its schema may not depend on its inputs",
                pins.tool_id
            ),
        ));
    }
    let limits = declaration.effective_limits(pins.tool_id, pins.max_input_bytes)?;
    let (inputs, input_revision) = read_inputs(&declaration, workspace, pins.tool_id, upstream)?;
    let schema = effective_schema(&declaration, &input_revision, &inputs, limits, pins.tool_id)?;
    let adoption = Arc::new(Adoption {
        declaration,
        tool_id: pins.tool_id.to_string(),
        tool_version: pins.version.to_string(),
        descriptor_sha256: pins.descriptor_sha256.to_string(),
        workspace: workspace.to_path_buf(),
        upstream: upstream.clone(),
        fixed_revision: input_revision.clone(),
        current: RwLock::new(Arc::new(AdoptedInputs {
            revision: input_revision,
            inputs,
        })),
        schema,
    });
    let held = adoptions().entry(key).or_insert(adoption).clone();
    held.verify_admission(pins, upstream)?;
    Ok(held)
}

impl Adoption {
    /// What is adopted now. One value, so a revision and its bytes cannot come from two versions.
    pub(crate) fn adopted(&self) -> Arc<AdoptedInputs> {
        self.current
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn schema(&self) -> &CompiledSchema {
        &self.schema
    }

    /// Every declared input, adopted yet or not.
    pub(crate) fn declared_inputs(&self) -> &[String] {
        &self.declaration.inputs
    }

    /// Takes up the version an upstream's commit left, and only that. The receipt says what the
    /// commit adopted, and the bytes there now must be those: a receipt alone, or bytes alone,
    /// moves nothing. A fixed input has no way through here.
    pub(crate) fn readopt(&self, receipt: &CommitReceipt) -> Result<(), ProfileToolRefusal> {
        let tool = &self.tool_id;
        let refused = |reason: &str| {
            ProfileToolRefusal::new(
                RefusalKind::InputRevisionMismatch,
                format!("stage=readopt tool `{tool}` {reason}"),
            )
        };
        if !receipt.committed || !self.upstream.contains(&receipt.path) {
            return Err(refused("was given a receipt that is not for its upstream"));
        }
        let bytes =
            read_without_following(&self.workspace, Path::new(&receipt.path), MAX_INPUT_BYTES)
                .map_err(|_| refused("cannot read the upstream's output"))?;
        if format!("{:x}", Sha256::digest(&bytes)) != receipt.committed_sha256 {
            return Err(refused(
                "found other bytes than the upstream's receipt names",
            ));
        }
        let mut current = self.current.write().unwrap_or_else(PoisonError::into_inner);
        let mut inputs = current.inputs.clone();
        inputs.retain(|input| input.path != receipt.path);
        inputs.push(SchemaInput {
            path: receipt.path.clone(),
            bytes,
        });
        if inputs.iter().map(|input| input.bytes.len()).sum::<usize>() > MAX_TOTAL_INPUT_BYTES {
            return Err(refused("would exceed the limit on its inputs"));
        }
        let position = |input: &SchemaInput| {
            self.declaration
                .inputs
                .iter()
                .position(|path| *path == input.path)
        };
        inputs.sort_by_key(position);
        *current = Arc::new(AdoptedInputs {
            revision: revision_of(&self.declaration.inputs, &inputs),
            inputs,
        });
        Ok(())
    }

    fn verify_admission(
        &self,
        pins: &ToolPins<'_>,
        upstream: &BTreeSet<String>,
    ) -> Result<(), ProfileToolRefusal> {
        if self.descriptor_sha256 != pins.descriptor_sha256
            || self.tool_version != pins.version
            || self.upstream != *upstream
        {
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
    /// give the adopted schema from the adopted bytes. An upstream input is not compared here:
    /// its change takes the state from this tool's stage, through the state machine.
    pub(crate) fn verify(&self, stage: &str) -> Result<(), ProfileToolRefusal> {
        let tool = &self.tool_id;
        let revision = read_inputs(&self.declaration, &self.workspace, tool, &self.upstream)
            .map(|(_, revision)| revision)
            .map_err(|refusal| {
                ProfileToolRefusal::new(
                    RefusalKind::InputRevisionMismatch,
                    format!("stage={stage} tool `{tool}`: {}", refusal.reason),
                )
            })?;
        if revision != self.fixed_revision {
            return Err(ProfileToolRefusal::new(
                RefusalKind::InputRevisionMismatch,
                format!("stage={stage} tool `{tool}` inputs changed since they were adopted"),
            ));
        }
        let current = self.adopted();
        let matches = effective_schema(
            &self.declaration,
            &current.revision,
            &current.inputs,
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

/// G3: the input paths a schema declares, without reading them, so the effect can judge
/// them first. The read of a `read` tool's inputs keeps its meaning.
pub(crate) fn declared_inputs(
    schema: &Value,
    tool: &str,
) -> Result<Vec<String>, ProfileToolRefusal> {
    Ok(SchemaDeclaration::decode(schema, tool)?.inputs)
}

/// What a stage set's admission needs from a schema, read without touching an input.
pub(crate) struct DeclaredShape {
    pub(crate) inputs: Vec<String>,
    pub(crate) has_compiler: bool,
    pub(crate) has_revision_argument: bool,
}

pub(crate) fn declared_shape(
    schema: &Value,
    tool: &str,
) -> Result<DeclaredShape, ProfileToolRefusal> {
    let declaration = SchemaDeclaration::decode(schema, tool)?;
    Ok(DeclaredShape {
        has_compiler: declaration.compiler.is_some(),
        has_revision_argument: declaration.revision_argument.is_some(),
        inputs: declaration.inputs,
    })
}

/// The revision is a digest of the declared inputs, in declared order: for each,
/// its path, a NUL, its length in decimal, a NUL, and its bytes. An upstream input with no
/// receipt yet has a word where its length would be, which no length can equal.
fn revision_of(declared: &[String], inputs: &[SchemaInput]) -> String {
    let mut digest = Sha256::new();
    for path in declared {
        digest.update(path.as_bytes());
        digest.update([0]);
        match inputs.iter().find(|input| input.path == *path) {
            Some(input) => {
                digest.update(input.bytes.len().to_string().as_bytes());
                digest.update([0]);
                digest.update(&input.bytes);
            }
            None => {
                digest.update(b"waiting");
                digest.update([0]);
            }
        }
    }
    format!("{:x}", digest.finalize())
}

fn read_inputs(
    declaration: &SchemaDeclaration,
    workspace: &Path,
    tool: &str,
    upstream: &BTreeSet<String>,
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
    for path in &declaration.inputs {
        if crate::tools::path_guard::validate_workspace_relative(path).is_err()
            || !seen.insert(path.as_str())
        {
            return Err(unavailable(
                "declares an input path that is not one distinct workspace-relative path"
                    .to_string(),
            ));
        }
        if upstream.contains(path) {
            continue;
        }
        let bytes = read_without_following(workspace, Path::new(path), MAX_INPUT_BYTES)
            .map_err(|reason| unavailable(format!("input {reason}")))?;
        total += bytes.len();
        if total > MAX_TOTAL_INPUT_BYTES {
            return Err(unavailable(format!(
                "inputs exceed {MAX_TOTAL_INPUT_BYTES} bytes"
            )));
        }
        inputs.push(SchemaInput {
            path: path.clone(),
            bytes,
        });
    }
    let revision = revision_of(&declaration.inputs, &inputs);
    Ok((inputs, revision))
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

#[cfg(test)]
mod readopt_tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};

    use super::super::super::RefusalKind;
    use super::super::super::commit::CommitReceipt;
    use super::{ToolPins, adopt, adopt_with_upstream};

    const UPSTREAM: &str = "out/upstream.json";
    const FIXED: &str = "fixed/seed.json";

    fn pins() -> ToolPins<'static> {
        ToolPins {
            tool_id: "reader-tool",
            version: "1",
            descriptor_sha256: "pin",
            max_input_bytes: 1 << 20,
        }
    }

    fn schema(inputs: &[&str]) -> Value {
        json!({
            "protocol": "profile-tool.schema/v1",
            "root": {"type": "object", "additionalProperties": false, "properties": {}},
            "inputs": inputs,
        })
    }

    fn workspace() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap();
        std::fs::create_dir_all(root.join("out")).unwrap();
        std::fs::create_dir_all(root.join("fixed")).unwrap();
        std::fs::write(root.join(FIXED), b"seed").unwrap();
        (temp, root)
    }

    fn receipt_for(path: &str, bytes: &[u8]) -> CommitReceipt {
        CommitReceipt {
            protocol: "profile-tool.commit-receipt/v1".to_string(),
            request_id: "request-readopt".to_string(),
            tool_id: "writer-tool".to_string(),
            output_id: "artifact".to_string(),
            path: path.to_string(),
            committed: true,
            durable: true,
            cancelled_after_commit: false,
            runtime_sha256: "a".repeat(64),
            input_revision: "b".repeat(64),
            schema_sha256: "c".repeat(64),
            result_sha256: "d".repeat(64),
            committed_sha256: format!("{:x}", Sha256::digest(bytes)),
            committed_bytes: bytes.len(),
            diagnostics: Vec::new(),
        }
    }

    fn upstream() -> BTreeSet<String> {
        BTreeSet::from([UPSTREAM.to_string()])
    }

    fn write(root: &Path, path: &str, bytes: &[u8]) {
        std::fs::write(root.join(path), bytes).unwrap();
    }

    #[test]
    fn a_new_upstream_version_is_taken_up_by_its_reader() {
        let (_temp, root) = workspace();
        // Before anything committed, the file is not there, and no refusal comes of it.
        let adoption = adopt_with_upstream(
            &schema(&[UPSTREAM, FIXED]),
            &pins(),
            &root,
            "readopt-profile",
            &upstream(),
        )
        .unwrap();
        let waiting = adoption.adopted();
        assert_eq!(waiting.inputs.len(), 1, "the fixed input alone");

        write(&root, UPSTREAM, b"version one");
        adoption.verify("before_execution").unwrap();
        assert_eq!(adoption.adopted(), waiting, "bytes alone move nothing");

        adoption
            .readopt(&receipt_for(UPSTREAM, b"version one"))
            .unwrap();
        let first = adoption.adopted();
        assert_ne!(first.revision, waiting.revision);
        assert_eq!(first.inputs[0].path, UPSTREAM, "in declared order");
        assert_eq!(first.inputs[0].bytes, b"version one");
        assert_eq!(first.inputs[1].path, FIXED);

        write(&root, UPSTREAM, b"version two");
        adoption.verify("before_execution").unwrap();
        assert_eq!(adoption.adopted(), first);
        adoption
            .readopt(&receipt_for(UPSTREAM, b"version two"))
            .unwrap();
        assert_ne!(adoption.adopted().revision, first.revision);

        // The same bytes again are the same revision: the generation is what tells them apart.
        write(&root, UPSTREAM, b"version one");
        adoption
            .readopt(&receipt_for(UPSTREAM, b"version one"))
            .unwrap();
        assert_eq!(adoption.adopted().revision, first.revision);
    }

    #[test]
    fn bytes_other_than_the_receipt_names_are_not_taken_up() {
        let (_temp, root) = workspace();
        let adoption = adopt_with_upstream(
            &schema(&[UPSTREAM]),
            &pins(),
            &root,
            "readopt-profile",
            &upstream(),
        )
        .unwrap();
        write(&root, UPSTREAM, b"version one");
        adoption
            .readopt(&receipt_for(UPSTREAM, b"version one"))
            .unwrap();
        let held = adoption.adopted();

        write(&root, UPSTREAM, b"rewritten by a command");
        let refusal = adoption
            .readopt(&receipt_for(UPSTREAM, b"version two"))
            .unwrap_err();
        assert_eq!(refusal.kind, RefusalKind::InputRevisionMismatch);
        assert_eq!(adoption.adopted(), held);

        std::fs::remove_file(root.join(UPSTREAM)).unwrap();
        let refusal = adoption
            .readopt(&receipt_for(UPSTREAM, b"version one"))
            .unwrap_err();
        assert_eq!(refusal.kind, RefusalKind::InputRevisionMismatch);
        assert_eq!(adoption.adopted(), held);

        // A receipt that says nothing was committed moves nothing either.
        write(&root, UPSTREAM, b"version three");
        let mut uncommitted = receipt_for(UPSTREAM, b"version three");
        uncommitted.committed = false;
        assert!(adoption.readopt(&uncommitted).is_err());
        assert_eq!(adoption.adopted(), held);
    }

    #[test]
    fn a_fixed_input_is_never_taken_up_again() {
        let (_temp, root) = workspace();
        let adoption = adopt_with_upstream(
            &schema(&[UPSTREAM, FIXED]),
            &pins(),
            &root,
            "readopt-profile",
            &upstream(),
        )
        .unwrap();
        let held = adoption.adopted();
        write(&root, FIXED, b"rewritten");
        let refusal = adoption
            .readopt(&receipt_for(FIXED, b"rewritten"))
            .unwrap_err();
        assert_eq!(refusal.kind, RefusalKind::InputRevisionMismatch);
        assert_eq!(adoption.adopted(), held);
        assert_eq!(
            adoption.verify("before_execution").unwrap_err().kind,
            RefusalKind::InputRevisionMismatch,
            "and it still ends the run"
        );

        // A tool outside a stage set has no way through at all.
        write(&root, FIXED, b"seed");
        let plain = adopt(&schema(&[FIXED]), &pins(), &root, "readopt-plain").unwrap();
        write(&root, FIXED, b"rewritten");
        assert!(plain.readopt(&receipt_for(FIXED, b"rewritten")).is_err());
        assert_eq!(
            plain.verify("before_execution").unwrap_err().kind,
            RefusalKind::InputRevisionMismatch
        );
    }

    #[test]
    fn a_later_admission_with_another_upstream_set_is_refused() {
        let (_temp, root) = workspace();
        adopt_with_upstream(
            &schema(&[UPSTREAM, FIXED]),
            &pins(),
            &root,
            "readopt-profile",
            &upstream(),
        )
        .unwrap();
        write(&root, UPSTREAM, b"present");
        let refusal = adopt(
            &schema(&[UPSTREAM, FIXED]),
            &pins(),
            &root,
            "readopt-profile",
        )
        .unwrap_err();
        assert_eq!(refusal.kind, RefusalKind::SchemaMismatch);
    }

    #[test]
    fn a_schema_that_depends_on_its_inputs_cannot_read_a_stage_output() {
        let (_temp, root) = workspace();
        let mut declared = schema(&[UPSTREAM]);
        declared["root"]["properties"] = json!({"revision": {"type": "string"}});
        declared["root"]["required"] = json!(["revision"]);
        declared["revision_argument"] = json!("revision");
        let refusal = adopt_with_upstream(&declared, &pins(), &root, "readopt-narrow", &upstream())
            .unwrap_err();
        assert_eq!(refusal.kind, RefusalKind::SchemaUnsupported);
    }
}
