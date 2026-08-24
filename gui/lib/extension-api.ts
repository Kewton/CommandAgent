import { apiPath } from "./base-path";
import { responseError } from "./errors";
import { trialAuthorizationHeaders } from "./trial-api";

export type PackLifecycleStatus = "staged" | "pinned" | "retired";

export type ExtensionPackSummary = {
  id: string;
  version: string;
  status: PackLifecycleStatus;
  hash: string | null;
  pin: string | null;
  conformance_ok: boolean;
  has_assist: boolean;
  has_eval: boolean;
  materials: string[];
  detail: string | null;
};

export type PackStageReport = {
  id: string;
  version: string;
  hash: string;
  status: PackLifecycleStatus;
  conformance: {
    status: string;
    profile: string;
    intent: string;
    floor_check_count: number;
    effective_check_count: number;
    schema_count: number;
  };
  scrub: {
    status: string;
    scanned: string[];
  };
  directory: string;
};

export type ExtensionPackDetail = {
  id: string;
  version: string;
  files: Record<string, string>;
  report: PackStageReport | {
    status: "failed" | "retired";
    id: string;
    version: string;
    hash: string | null;
    reason?: string;
  };
};

export async function listExtensionPacks(token: string): Promise<ExtensionPackSummary[]> {
  return extensionJson<ExtensionPackSummary[]>("extensions/packs", token);
}

export async function fetchExtensionPack(
  token: string,
  id: string,
  version: string,
): Promise<ExtensionPackDetail> {
  return extensionJson<ExtensionPackDetail>(packPath(id, version), token);
}

export async function stageExtensionPack(
  token: string,
  request: { id: string; version: string; files: Record<string, string> },
): Promise<PackStageReport> {
  return extensionJson<PackStageReport>("extensions/packs", token, {
    method: "POST",
    headers: trialAuthorizationHeaders(token, true),
    body: JSON.stringify(request),
  });
}

export async function verifyExtensionPack(
  token: string,
  id: string,
  version: string,
): Promise<PackStageReport> {
  return extensionJson<PackStageReport>(`${packPath(id, version)}/verify`, token, {
    method: "POST",
    headers: trialAuthorizationHeaders(token),
  });
}

export async function pinExtensionPack(
  token: string,
  id: string,
  version: string,
  hash: string,
): Promise<void> {
  await extensionOk(`${packPath(id, version)}/pin`, token, {
    method: "POST",
    headers: trialAuthorizationHeaders(token, true),
    body: JSON.stringify({ hash }),
  });
}

export async function retireExtensionPack(
  token: string,
  id: string,
  version: string,
): Promise<void> {
  await extensionOk(`${packPath(id, version)}/retire`, token, {
    method: "POST",
    headers: trialAuthorizationHeaders(token),
  });
}

function packPath(id: string, version: string): string {
  return `extensions/packs/${encodeURIComponent(id)}/${encodeURIComponent(version)}`;
}

async function extensionJson<T>(
  resource: string,
  token: string,
  init?: RequestInit,
): Promise<T> {
  const response = await fetch(apiPath(resource), {
    ...init,
    headers: init?.headers ?? trialAuthorizationHeaders(token),
  });
  if (!response.ok) throw await responseError(response);
  return (await response.json()) as T;
}

async function extensionOk(
  resource: string,
  token: string,
  init: RequestInit,
): Promise<void> {
  const response = await fetch(apiPath(resource), {
    ...init,
    headers: init.headers ?? trialAuthorizationHeaders(token),
  });
  if (!response.ok) throw await responseError(response);
}
