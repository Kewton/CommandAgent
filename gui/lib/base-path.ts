const BASE_PATH = normalizeBasePath(process.env.NEXT_PUBLIC_GUI_BASE_PATH);

export type GuiRoute = "dashboard" | "try" | "run" | "assets" | "measurements";

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

export function routePath(route: GuiRoute, runId?: string): string {
  switch (route) {
    case "dashboard":
      return "/";
    case "try":
      return "/try/";
    case "run":
      return `/runs/?id=${encodeURIComponent(runId ?? "")}`;
    case "assets":
      return "/assets/";
    case "measurements":
      return "/measurements/";
  }
}

export function apiPath(resource: string, query?: URLSearchParams): string {
  const suffix = query === undefined ? "" : `?${query.toString()}`;
  return withBasePath(`/api/${resource}${suffix}`);
}
