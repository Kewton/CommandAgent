//! G2 (Issue #973): nested arguments checked against a schema bound to an input
//! revision, at the host's receiving boundary and again right before an execution.
//!
//! - [`admission`] compiles the closed subset and refuses what it does not define.
//! - [`raw_json`] decodes raw argument text, where a repeated key still exists.
//! - [`validate`] is the one validator receipt and the execution boundary share.
//! - [`binding`] adopts a tool's inputs and schema once and compares them after.
//! - [`invocation`] carries what a session admitted to the receiving parser.
//!
//! All of this applies only to a descriptor that declares `schema`. Without one,
//! nothing here reads a file, decodes a reply, or changes a value.

pub(crate) mod admission;
pub(crate) mod binding;
pub(crate) mod invocation;
pub(crate) mod raw_json;
pub(crate) mod validate;

use serde::Deserialize;
use serde_json::Value;

use super::{ProfileToolRefusal, RefusalKind};

pub use binding::{
    ProfileToolSchemaCompiler, SchemaCompileInput, SchemaInput, register_schema_compiler,
};

pub const SCHEMA_PROTOCOL: &str = "profile-tool.schema/v1";

/// Branches of one `anyOf`.
pub(crate) const MAX_ANY_OF_BRANCHES: usize = 16;
/// The product of the `anyOf` branches along any path through a schema: how many
/// times one argument value can be tried. With 512 elements, validation visits at
/// most 512 × 4096 schema nodes.
pub(crate) const MAX_UNION_PATHS: usize = 4096;
pub(crate) const MAX_PATTERN_BYTES: usize = 512;
/// The compiled size of one pattern, in the `regex` crate's own bound.
pub(crate) const MAX_REGEX_BYTES: usize = 1 << 20;
pub(crate) const MAX_ENUM_VALUE_BYTES: usize = 1024;
/// A schema's own JSON nesting: two levels per object, one per array, two per
/// `anyOf`. This bounds recursion; `Limits::depth` is what it describes.
pub(crate) const MAX_SCHEMA_JSON_DEPTH: usize = 96;
pub(crate) const MAX_DIAGNOSTICS: usize = 8;

/// The opening limits of this protocol. An operator can only narrow them. They are
/// separate from the artifact-contract v1 limits, which are unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Limits {
    pub(crate) argument_bytes: usize,
    pub(crate) depth: usize,
    pub(crate) elements: usize,
    pub(crate) schema_bytes: usize,
}

impl Limits {
    pub(crate) const OPENING: Self = Self {
        argument_bytes: 64 * 1024,
        depth: 32,
        elements: 512,
        schema_bytes: 256 * 1024,
    };
}

/// The `schema` member of a descriptor, covered by its `schema_sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SchemaDeclaration {
    pub(crate) protocol: String,
    pub(crate) root: Value,
    #[serde(default)]
    pub(crate) inputs: Vec<String>,
    #[serde(default)]
    pub(crate) compiler: Option<String>,
    #[serde(default)]
    pub(crate) revision_argument: Option<String>,
    #[serde(default)]
    pub(crate) limits: NarrowedLimits,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NarrowedLimits {
    pub(crate) max_argument_bytes: Option<usize>,
    pub(crate) max_depth: Option<usize>,
    pub(crate) max_elements: Option<usize>,
    pub(crate) max_schema_bytes: Option<usize>,
}

impl SchemaDeclaration {
    pub(crate) fn decode(value: &Value, tool: &str) -> Result<Self, ProfileToolRefusal> {
        let declaration: Self = serde_json::from_value(value.clone()).map_err(|error| {
            ProfileToolRefusal::new(
                RefusalKind::SchemaUnsupported,
                format!("tool `{tool}` schema declaration does not decode: {error}"),
            )
        })?;
        if declaration.protocol != SCHEMA_PROTOCOL {
            return Err(ProfileToolRefusal::new(
                RefusalKind::SchemaUnsupported,
                format!("tool `{tool}` schema protocol must be `{SCHEMA_PROTOCOL}`"),
            ));
        }
        Ok(declaration)
    }

    /// The opening limits, narrowed by this declaration and by the input limit the
    /// tool was admitted with. A value above an opening limit is refused rather
    /// than clamped.
    pub(crate) fn effective_limits(
        &self,
        tool: &str,
        max_input_bytes: usize,
    ) -> Result<Limits, ProfileToolRefusal> {
        let narrowed = |value: Option<usize>, opening: usize, label: &str| match value {
            Some(value) if value == 0 || value > opening => Err(ProfileToolRefusal::new(
                RefusalKind::SchemaLimitExceeded,
                format!("tool `{tool}` {label} can only narrow the limit, within 1..{opening}"),
            )),
            Some(value) => Ok(value),
            None => Ok(opening),
        };
        let opening = Limits::OPENING;
        Ok(Limits {
            argument_bytes: narrowed(
                self.limits.max_argument_bytes,
                opening.argument_bytes,
                "max_argument_bytes",
            )?
            .min(max_input_bytes),
            depth: narrowed(self.limits.max_depth, opening.depth, "max_depth")?,
            elements: narrowed(self.limits.max_elements, opening.elements, "max_elements")?,
            schema_bytes: narrowed(
                self.limits.max_schema_bytes,
                opening.schema_bytes,
                "max_schema_bytes",
            )?,
        })
    }
}
