export type RunState = "pass" | "fail" | "pending" | "unknown";

export type RunSummary = {
  id: string;
  modified_epoch_seconds: number;
  report_path: string | null;
  status: string;
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
  pin: string;
  has_assist: boolean;
  has_eval: boolean;
};

export type SessionSpec = {
  goal: string;
  profile: string;
  provider: string;
  model: string;
  planner_provider: string;
  planner_model: string;
};

export type ConfirmationIdentity = {
  request: string;
  workspace: string;
  profile: string;
  intent: string;
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
  pins: {
    planner_provider: string;
    planner_model: string;
    executor_provider: string;
    executor_model: string;
    preset: string;
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
  gate: "gate_2";
  status: "starting";
  events_path: string;
};

export type PhaseStatus = {
  id: string;
  index: number;
  total: number;
  stage: string;
  status: string;
};

export type PolledSession = {
  id: string;
  gate: "gate_2" | "gate_3" | "gate_4";
  status: string;
  verdict: string | null;
  assurance: string | null;
  phases: PhaseStatus[];
  event_count: number;
  acceptance_sheet: string | null;
  section5: string | null;
  events_path: string;
};

export type DirectiveProposal = {
  directive_hash: string;
  directive_round: number;
  issued_gate: "gate_3" | "gate_4";
  scrubbed_directive: string;
  confirmation_required: boolean;
};
