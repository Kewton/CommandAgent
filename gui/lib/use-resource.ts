"use client";

import { useCallback, useEffect, useState } from "react";

import { apiPath } from "./base-path";
import { describeError, responseError } from "./errors";

type ResourceState<T> = {
  data: T | null;
  error: string | null;
  loading: boolean;
};

type RefreshableResourceState<T> = ResourceState<T> & {
  refresh: () => void;
};

export function useResource<T>(resource: string): RefreshableResourceState<T> {
  const [revision, setRevision] = useState(0);
  const [state, setState] = useState<ResourceState<T>>({
    data: null,
    error: null,
    loading: true,
  });
  const refresh = useCallback(() => setRevision((current) => current + 1), []);

  useEffect(() => {
    let cancelled = false;
    let requestInFlight = false;
    let refreshWhenSettled = false;
    let controller: AbortController | undefined;

    setState({ data: null, error: null, loading: true });

    const revalidate = async (afterCurrent = false) => {
      if (cancelled) return;
      if (requestInFlight) {
        refreshWhenSettled ||= afterCurrent;
        return;
      }

      requestInFlight = true;
      refreshWhenSettled = false;
      controller = new AbortController();
      setState((current) => ({ ...current, error: null, loading: true }));
      try {
        const response = await fetch(apiPath(resource), {
          cache: "no-store",
          signal: controller.signal,
        });
        if (!response.ok) {
          throw await responseError(response);
        }
        const data = (await response.json()) as T;
        if (!cancelled) setState({ data, error: null, loading: false });
      } catch (error: unknown) {
        if (error instanceof DOMException && error.name === "AbortError") return;
        if (!cancelled) {
          setState((current) => ({
            data: current.data,
            error: describeError(error),
            loading: false,
          }));
        }
      } finally {
        requestInFlight = false;
        controller = undefined;
        if (!cancelled && refreshWhenSettled && document.visibilityState === "visible") {
          refreshWhenSettled = false;
          void revalidate();
        }
      }
    };

    const refresh = () => {
      if (document.visibilityState === "visible") void revalidate();
    };
    const refreshWhenVisible = () => {
      if (document.visibilityState === "visible") void revalidate(true);
    };

    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refreshWhenVisible);
    void revalidate();

    return () => {
      cancelled = true;
      controller?.abort();
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refreshWhenVisible);
    };
  }, [resource, revision]);

  return { ...state, refresh };
}
