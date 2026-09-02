"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { trialRoutePath, type TrialRoute } from "../lib/base-path";

const items: Array<{ route: TrialRoute; label: string; detail: string }> = [
  { route: "compose", label: "実行指示", detail: "Gate 1" },
  { route: "status", label: "実行状況", detail: "進行中" },
  { route: "history", label: "実行履歴", detail: "一覧" },
  { route: "detail", label: "結果詳細", detail: "Gate 3 / 4" },
];

export function TrialPageNavigation({ active }: { active: TrialRoute }) {
  const [sessionId, setSessionId] = useState<string | undefined>();

  useEffect(() => {
    const value = new URLSearchParams(window.location.search).get("session")?.trim();
    if (value !== undefined && value !== "") setSessionId(value);
  }, []);

  return (
    <nav aria-label="トライアルページ" className="trial-page-nav" data-testid="trial-page-nav">
      {items.map((item, index) => (
        <Link
          aria-current={item.route === active ? "page" : undefined}
          className={item.route === active ? "active" : undefined}
          data-testid={`trial-page-nav-${item.route}`}
          href={trialRoutePath(
            item.route,
            item.route === "status" || item.route === "detail" ? sessionId : undefined,
          )}
          key={item.route}
        >
          <span>{String(index + 1).padStart(2, "0")}</span>
          <strong>{item.label}</strong>
          <small>{item.detail}</small>
        </Link>
      ))}
    </nav>
  );
}
