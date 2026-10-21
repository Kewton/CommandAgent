//! Result types shared by the planner and the minimal loop.
//!
//! This module is distinct from `crate::verification_spec`, which declares
//! verification specifications. It only holds the pure data that a verifier
//! produces (`VerificationReport`, `VerifyStatus`, and the diagnostics a report
//! carries) so that `planner` and `minimal_loop` need not reference each other
//! for these types. Running, probing, and repairing stay in their original
//! modules; the old paths re-export these types.

pub mod build_report;
pub mod diagnostics;
pub mod report;
