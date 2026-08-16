"use client";

import { useEffect, useState } from "react";

import { apiPath } from "./base-path";
import { describeError, responseError } from "./errors";

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
          throw await responseError(response);
        }
        return response.json() as Promise<T>;
      })
      .then((data) => setState({ data, error: null, loading: false }))
      .catch((error: unknown) => {
        if (error instanceof DOMException && error.name === "AbortError") return;
        setState({
          data: null,
          error: describeError(error),
          loading: false,
        });
      });
    return () => controller.abort();
  }, [resource]);

  return state;
}
