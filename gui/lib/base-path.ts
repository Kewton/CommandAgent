const BASE_PATH = normalizeBasePath(process.env.NEXT_PUBLIC_GUI_BASE_PATH);

export type GuiRoute = "dashboard" | "try" | "run" | "assets" | "measurements";
export type TrialRoute = "compose" | "status" | "history" | "detail";

export function guiBasePath(): string {
  return BASE_PATH;
}

function normalizeBasePath(value: string | undefined): string {
  if (value === undefined || value === "" || value === "/") {
    return "";
  }
  const withLeadingSlash = value.startsWith("/") ? value : `/${value}`;
  return withLeadingSlash.replace(/\/+$/, "");
}

export function withBasePath(path: string): string {
  const normalized = path.startsWith("/") ? path : `/${path}`;
  return `${BASE_PATH}${normalized}`;
}

export function routePath(route: GuiRoute, resourceId?: string): string {
  switch (route) {
    case "dashboard":
      return "/";
    case "try":
      return resourceId === undefined ? trialRoutePath("compose") : trialRoutePath("status", resourceId);
    case "run":
      return `/runs/?id=${encodeURIComponent(resourceId ?? "")}`;
    case "assets":
      return "/assets/";
    case "measurements":
      return "/measurements/";
  }
}

export function trialRoutePath(route: TrialRoute, sessionId?: string): string {
  const path = route === "compose"
    ? "/try/"
    : route === "status"
      ? "/try/status/"
      : route === "history"
        ? "/try/history/"
        : "/try/history/detail/";
  return sessionId === undefined || sessionId.trim() === ""
    ? path
    : `${path}?session=${encodeURIComponent(sessionId)}`;
}

export function apiPath(resource: string, query?: URLSearchParams): string {
  const suffix = query === undefined ? "" : `?${query.toString()}`;
  return withBasePath(`/api/${resource}${suffix}`);
}
