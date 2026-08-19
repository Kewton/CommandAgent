"use client";

import { useEffect, useState } from "react";

import { apiPath } from "./base-path";
import { responseError } from "./errors";
import type { RuntimeStatus } from "./types";

export type RuntimeState = {
  data: RuntimeStatus | null;
  failed: boolean;
};

const REFRESH_INTERVAL_MS = 3_000;

export function useRuntimeStatus(): RuntimeState {
  const [state, setState] = useState<RuntimeState>({ data: null, failed: false });

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let requestInFlight = false;
    let refreshWhenSettled = false;
    let controller: AbortController | undefined;

    const clearRefreshTimer = () => {
      if (timer === undefined) return;
      clearTimeout(timer);
      timer = undefined;
    };
    const documentIsHidden = () => document.visibilityState === "hidden";

    const refresh = async () => {
      if (cancelled || documentIsHidden()) return;
      if (requestInFlight) {
        refreshWhenSettled = true;
        return;
      }

      clearRefreshTimer();
      requestInFlight = true;
      controller = new AbortController();
      try {
        const response = await fetch(apiPath("runtime-status"), {
          cache: "no-store",
          signal: controller.signal,
        });
        if (!response.ok) throw await responseError(response);
        const data = (await response.json()) as RuntimeStatus;
        if (!cancelled) setState({ data, failed: false });
      } catch (reason) {
        if (reason instanceof DOMException && reason.name === "AbortError") return;
        if (!cancelled) setState((current) => ({ data: current.data, failed: true }));
      } finally {
        requestInFlight = false;
        controller = undefined;
        if (cancelled || documentIsHidden()) return;
        if (refreshWhenSettled) {
          refreshWhenSettled = false;
          void refresh();
        } else {
          timer = setTimeout(() => void refresh(), REFRESH_INTERVAL_MS);
        }
      }
    };

    const refreshWhenVisible = () => {
      clearRefreshTimer();
      if (documentIsHidden()) {
        refreshWhenSettled = false;
        controller?.abort();
        return;
      }
      void refresh();
    };

    document.addEventListener("visibilitychange", refreshWhenVisible);
    void refresh();
    return () => {
      cancelled = true;
      clearRefreshTimer();
      controller?.abort();
      document.removeEventListener("visibilitychange", refreshWhenVisible);
    };
  }, []);

  return state;
}
