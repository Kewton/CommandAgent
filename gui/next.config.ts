import type { NextConfig } from "next";

function normalizeBasePath(value: string | undefined): string {
  if (value === undefined || value === "" || value === "/") {
    return "";
  }
  const withLeadingSlash = value.startsWith("/") ? value : `/${value}`;
  return withLeadingSlash.replace(/\/+$/, "");
}

const basePath = normalizeBasePath(process.env.GUI_BASE_PATH);

const nextConfig: NextConfig = {
  output: "export",
  basePath,
  trailingSlash: true,
  env: {
    NEXT_PUBLIC_GUI_BASE_PATH: basePath,
  },
};

export default nextConfig;
