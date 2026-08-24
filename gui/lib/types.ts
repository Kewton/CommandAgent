export type RunState = "pass" | "fail" | "pending" | "unknown";

export type RunSummary = {
  id: string;
  modified_epoch_seconds: number;
  report_path: string | null;
  status: string;
  status_text: string;
  state: RunState;
};

export type RunIndex = {
  runs: RunSummary[];
  total: number;
};

export type DocumentRecord = {
  id: string;
  path: string;
  content: string;
};

export type DocumentSummary = {
  id: string;
  path: string;
  size_bytes: number;
};

export type RunDetail = {
  id: string;
  acceptance_path: string | null;
  acceptance: string;
  evidence: DocumentSummary[];
};

export type PackSummary = {
  id: string;
  version: string;
  path: string;
  profile: string | null;
  intent: string | null;
  source: "admitted" | "repository" | "local";
  source_label: string;
  pin: string;
  expected_hash: string | null;
  observed_hash: string | null;
  hash_matches_pin: boolean;
  has_assist: boolean;
  has_eval: boolean;
  retired: boolean;
  shadowing_repository: boolean;
  trial_eligible: boolean;
  warning: string | null;
};

export type OllamaThink = "true" | "false" | "low" | "medium" | "high";
export type TrialIntent = "create" | "fix" | "investigate";

export type SessionSpec = {
  goal: string;
  profile: string;
  intent: TrialIntent | null;
  provider: string;
  model: string;
  planner_provider: string;
  planner_model: string;
  pack: string | null;
  think: OllamaThink | null;
};

export type PackOption = {
  id: string;
  version: string;
  profile: string;
  intent: TrialIntent;
  hash: string;
  point: string;
  source: "admitted" | "repository" | "local";
  source_label: string;
};

export type PackOptions = {
  packs: PackOption[];
};

export type TrialOptions = {
  profiles: Array<{
    id: string;
    label: string;
    description: string;
    status: "admitted" | "draft";
    manifest_hash: string | null;
    assurance_ceiling: "full" | "static";
    base_profile: string | null;
  }>;
  providers: Array<{
    id: string;
    label: string;
    model_hint: string;
  }>;
};

export type ConfirmationIdentity = {
  request: string;
  workspace: string;
  profile: string;
  intent: TrialIntent;
  task_family: string;
  route_bases: string[];
  contract_ref: string;
  contract_checks: string[];
  band_full: number;
  band_denominator: number;
  band_rate: string;
  band_arm: string;
  band_measurement: string;
  band_source: string;
  full_meaning: string;
  draft_manifest?: {
    source: "repository" | "local";
    path: string;
    hash: string;
    assurance_ceiling: "static";
    base_profile?: string;
  };
  pins: {
    planner_provider: string;
    planner_model: string;
    executor_provider: string;
    executor_model: string;
    preset: string;
    think?: OllamaThink;
  };
  pack:
    | { selection: "none" }
    | {
        selection: "pinned";
        id: string;
        version: string;
        hash: string;
        point: string;
        source: "admitted" | "repository" | "local";
      };
};

export type SessionProposal = {
  confirmation_required: boolean;
  card_hash: string;
  card_markdown: string;
  identity: ConfirmationIdentity;
  price: {
    duration_n: number;
    average_duration_seconds: number | null;
    cost_n: number;
    average_cost_usd: number | null;
    source: string;
  };
};

export type CreatedSession = {
  id: string;
  started_epoch_seconds: number;
  gate: "gate_2";
  status: "starting";
  events_path: string;
};

export type SessionPathProjection = {
  id: string;
  working_directory: {
    path: string;
    state: "available" | "missing";
  };
  run_records: {
    directory: string;
    events: string;
    summary: string;
  };
};

export type TrialWorkspaceLease =
  | { status: "idle" }
  | { status: "running"; session_id: string }
  | { status: "recovery_required"; session_id: string };

export type TrialSessionSummary = {
  id: string;
  started_epoch_seconds: number;
  modified_epoch_seconds: number;
  gate: "gate_2" | "gate_3" | "gate_4" | null;
  status: string;
  profile?: string | null;
  intent?: TrialIntent | string | null;
  failure_diagnostics?: FailureDiagnostics;
  pack: {
    id: string;
    version: string;
    hash: string;
    source: "admitted" | "repository" | "local";
    source_label: string;
  } | null;
};

export type TrialSessionIndex = {
  sessions: TrialSessionSummary[];
  lease: TrialWorkspaceLease;
};

export type PhaseStatus = {
  id: string;
  index: number;
  total: number;
  stage: string;
  status: string;
};

export type PlanTaskStatus = {
  step_execution_id: string;
  step_index: number;
  total_steps: number;
  step_id: string;
  step_kind: string;
  status: "running" | "completed" | "short_circuited" | "failed" | "interrupted";
  outcome: string | null;
  verification_status: string | null;
  verification_failure_count: number;
  verification_failures: string[];
  verification_failures_truncated: boolean;
  changed_path_count: number;
  changed_paths: string[];
  changed_paths_truncated: boolean;
  repair_attempts: number;
  failure_summary: string | null;
};

export type PlanTaskExecution = {
  execution_index: number;
  plan_execution_id: string;
  mode: string;
  phase_id: string | null;
  total_steps: number;
  tasks: PlanTaskStatus[];
};

export type TaskProgress = {
  status: "pending" | "supported" | "unsupported";
  executions: PlanTaskExecution[];
};

export type FailureDiagnostics = {
  stop_reason: string | null;
  release_gate_reasons: string[];
  probe_findings: Array<{
    name: string;
    status: string | null;
    reasons: string[];
    evidence_path: string | null;
  }>;
};

export type BoundedText = {
  value: string;
  truncated: boolean;
};

export type BoundedTextList = {
  items: BoundedText[];
  total_count: number;
  truncated: boolean;
};

export type FailureExplanation = {
  projection_status: "supported" | "fallback";
  category:
    | "planning"
    | "execution"
    | "verification"
    | "release_gate"
    | "infrastructure"
    | "interrupted"
    | "unknown";
  location: {
    interval_index: number;
    plan_execution_id: BoundedText | null;
    phase: { id: BoundedText; index: number | null; total: number | null } | null;
    step: {
      execution_id: BoundedText;
      id: BoundedText;
      kind: BoundedText;
      index: number;
      total: number;
    } | null;
  };
  primary: {
    summary: BoundedText;
    failure_kind: BoundedText | null;
    reason_code: BoundedText | null;
  };
  evidence: {
    command: BoundedText | null;
    exit_code: number | null;
    stdout: BoundedText | null;
    stderr: BoundedText | null;
    verification_status: BoundedText | null;
    acceptance_status: BoundedText | null;
    release_gate_status: BoundedText | null;
    observations: Array<{
      kind: BoundedText;
      status: BoundedText | null;
      detail: BoundedText | null;
      path: BoundedText | null;
    }>;
    observation_count: number;
    observations_truncated: boolean;
    missing_paths: BoundedTextList;
    changed_paths: BoundedTextList;
    evidence_paths: BoundedTextList;
  };
  progress: {
    completed_phases: number;
    total_phases: number;
    completed_tasks: number;
    total_tasks: number;
    repair_attempts: number;
    workspace_state: "available" | "missing" | "unknown";
    partial_artifact_state: "observed" | "workspace_available" | "workspace_missing" | "unknown";
  };
  recovery: {
    next_action_code: BoundedText | null;
    explanation: BoundedText;
    viable_actions: BoundedTextList;
    repair_prompt_path: BoundedText | null;
    recovery_plan_path: BoundedText | null;
    suggested_command: BoundedText | null;
    suggested_yaml_command: BoundedText | null;
    continuation_eligible: boolean;
    continuation_reason: BoundedText;
  };
  technical: {
    machine_codes: BoundedTextList;
  };
};

export type PolledSession = {
  id: string;
  started_epoch_seconds: number;
  average_duration_seconds: number | null;
  gate: "gate_2" | "gate_3" | "gate_4";
  status: string;
  verdict: string | null;
  assurance: string | null;
  assurance_reason: string | null;
  stop_reason: string | null;
  failure_diagnostics?: FailureDiagnostics;
  failure_explanation?: FailureExplanation | null;
  next_action: string | null;
  phases: PhaseStatus[];
  task_progress: TaskProgress;
  event_count: number;
  acceptance_sheet: string | null;
  section5: string | null;
  events_path: string;
  identity?: ConfirmationIdentity;
};

export type DirectiveProposal = {
  directive_hash: string;
  directive_round: number;
  issued_gate: "gate_3" | "gate_4";
  scrubbed_directive: string;
  confirmation_required: boolean;
};

export type RuntimeStatus = {
  gui_contract_version?: string;
  trial_available: boolean;
  trial_token_auth_enabled: boolean;
  prerequisites: {
    execution_root: RuntimePrerequisite;
    commandagent_binary: RuntimePrerequisite;
    trial_authentication: RuntimePrerequisite;
  };
  session: {
    id: string;
    state: "running" | "recovery_required";
  } | null;
};

export type RuntimePrerequisite = {
  status: "ready" | "unconfigured" | "action_required";
  detail: string;
};
