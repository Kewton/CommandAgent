"use client";

import { useEffect, useState } from "react";

import { apiPath } from "./base-path";
import { responseError } from "./errors";
import type { RuntimeStatus } from "./types";

type RuntimeState = {
  data: RuntimeStatus | null;
  failed: boolean;
};

const REFRESH_INTERVAL_MS = 3_000;

export function useRuntimeStatus(): RuntimeState {
  const [state, setState] = useState<RuntimeState>({ data: null, failed: false });

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const refresh = async () => {
      try {
        const response = await fetch(apiPath("runtime-status"), { cache: "no-store" });
        if (!response.ok) throw await responseError(response);
        const data = (await response.json()) as RuntimeStatus;
        if (!cancelled) setState({ data, failed: false });
      } catch {
        if (!cancelled) setState((current) => ({ data: current.data, failed: true }));
      } finally {
        if (!cancelled) timer = setTimeout(() => void refresh(), REFRESH_INTERVAL_MS);
      }
    };
    void refresh();
    return () => {
      cancelled = true;
      if (timer !== undefined) clearTimeout(timer);
    };
  }, []);

  return state;
}
