export type RunSummary = {
  id: string;
  modified_epoch_seconds: number;
  report_path: string | null;
  status: string;
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
