use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixFailureClassification {
    /// Schema-v1 default, omitted from serialized evidence to preserve ordinary fix bytes.
    #[default]
    SubjectFailure,
    /// Explicit only when the reproducer fails before evaluating its subject.
    ReproducerDefect,
}

impl FixFailureClassification {
    pub const fn is_subject(&self) -> bool {
        matches!(self, Self::SubjectFailure)
    }

    pub const fn is_reproducer_defect(self) -> bool {
        matches!(self, Self::ReproducerDefect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subject_failure_is_the_byte_compatible_default() {
        #[derive(Deserialize, Serialize)]
        struct Record {
            #[serde(default, skip_serializing_if = "FixFailureClassification::is_subject")]
            failure_classification: FixFailureClassification,
        }

        assert_eq!(
            serde_json::to_string(&Record {
                failure_classification: FixFailureClassification::SubjectFailure,
            })
            .unwrap(),
            "{}"
        );
        assert_eq!(
            serde_json::to_string(&Record {
                failure_classification: FixFailureClassification::ReproducerDefect,
            })
            .unwrap(),
            r#"{"failure_classification":"reproducer_defect"}"#
        );
        let legacy: Record = serde_json::from_str("{}").unwrap();
        assert_eq!(
            legacy.failure_classification,
            FixFailureClassification::SubjectFailure
        );
    }
}
