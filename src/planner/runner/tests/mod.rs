use super::*;
use crate::planner::lint::lint_step_plan_report_with_workspace;
use crate::providers::{AssistantReply, ChatClient};
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

#[path = "final_acceptance_tests.rs"]
mod final_acceptance_tests;

#[path = "ultra_plan_flow_tests.rs"]
mod ultra_plan_flow_tests;

include!("recovery_host_verification_tests.rs");
#[path = "data_pre_satisfied_tests.rs"]
mod data_pre_satisfied_tests;

#[path = "assurance_tests.rs"]
mod assurance_tests;

#[path = "cli_runtime_dispatch_tests.rs"]
mod cli_runtime_dispatch_tests;

#[path = "requested_port_tests.rs"]
mod requested_port_tests;

#[path = "profile_runtime_tests.rs"]
mod profile_runtime_tests;

include!("support/text.rs");
include!("driver_tests.rs");
include!("phase_runtime_tests.rs");
include!("acceptance_boundary_tests.rs");
include!("step_repair_tests.rs");
include!("support/client.rs");
include!("support/plan.rs");
include!("support/nextjs.rs");
include!("support/browser.rs");
include!("support/evidence.rs");
include!("support/build.rs");
include!("compile_repair_tests.rs");
include!("support/process.rs");
