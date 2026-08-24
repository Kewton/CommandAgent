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
  next_action: string | null;
  phases: PhaseStatus[];
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
