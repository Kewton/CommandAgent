"use client";

import { useEffect, useState } from "react";

import { apiPath } from "./base-path";

type ResourceState<T> = {
  data: T | null;
  error: string | null;
  loading: boolean;
};

export function useResource<T>(resource: string): ResourceState<T> {
  const [state, setState] = useState<ResourceState<T>>({
    data: null,
    error: null,
    loading: true,
  });

  useEffect(() => {
    const controller = new AbortController();
    setState({ data: null, error: null, loading: true });
    fetch(apiPath(resource), { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          const message = await response.text();
          throw new Error(`${response.status}: ${message}`);
        }
        return response.json() as Promise<T>;
      })
      .then((data) => setState({ data, error: null, loading: false }))
      .catch((error: unknown) => {
        if (error instanceof DOMException && error.name === "AbortError") return;
        setState({
          data: null,
          error: error instanceof Error ? error.message : "Unknown API error",
          loading: false,
        });
      });
    return () => controller.abort();
  }, [resource]);

  return state;
}
