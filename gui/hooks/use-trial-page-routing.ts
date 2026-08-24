"use client";

import { useEffect, useRef } from "react";

import { trialRoutePath, withBasePath, type TrialRoute } from "../lib/base-path";
import type { ScreenStage } from "./use-trial-compose";

export function useTrialPageRouting(
  surface: TrialRoute,
  stage: ScreenStage,
  sessionId: string | null,
) {
  const lastRedirect = useRef<string | null>(null);

  useEffect(() => {
    if (surface === "history" || sessionId === null) return;
    const targetRoute = stage === "gate_2"
      ? "status"
      : stage === "terminal" || stage === "closed"
        ? "detail"
        : null;
    if (targetRoute === null || targetRoute === surface) return;
    const target = withBasePath(trialRoutePath(targetRoute, sessionId));
    if (lastRedirect.current === target) return;
    lastRedirect.current = target;
    window.location.replace(target);
  }, [sessionId, stage, surface]);
}
