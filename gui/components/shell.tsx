"use client";

import Link from "next/link";
import { createContext, useContext, type ReactNode } from "react";

import { routePath, type GuiRoute } from "../lib/base-path";
import { useRuntimeStatus, type RuntimeState } from "../lib/use-runtime-status";

const RuntimeStatusContext = createContext<RuntimeState | null>(null);

export function useShellRuntimeStatus(): RuntimeState | null {
  return useContext(RuntimeStatusContext);
}

const navigation: { route: GuiRoute; label: string; index: string }[] = [
  { route: "dashboard", label: "概要", index: "01" },
  { route: "try", label: "トライアル", index: "02" },
  { route: "assets", label: "拡張", index: "03" },
  { route: "run", label: "リポジトリ実行記録", index: "04" },
  { route: "measurements", label: "計測", index: "05" },
];

type ShellProps = {
  active: GuiRoute;
  title: string;
  description: string;
  children: ReactNode;
};

export function Shell({ active, title, description, children }: ShellProps) {
  const runtime = useRuntimeStatus();
  const runtimeSession = runtime.data?.session ?? null;
  const sessionState = runtimeSession?.state ?? "idle";
  const sessionLabel = runtime.failed
    ? "状態取得失敗"
    : runtimeSession?.state === "running"
      ? `実行中 ${shortSessionId(runtimeSession.id)}`
      : runtimeSession?.state === "recovery_required"
        ? `要復旧 ${shortSessionId(runtimeSession.id)}`
        : "実行中なし";

  return (
    <RuntimeStatusContext.Provider value={runtime}>
      <div className="app-shell">
      <header className="topbar">
        <Link className="brand" href={routePath("dashboard")}>
          <span className="brand-mark" aria-hidden="true">
            CA
          </span>
          <span>
            <strong>CommandAgent</strong>
            <small>運用オブザーバトリ</small>
          </span>
        </Link>
        <div
          aria-atomic="true"
          aria-live="polite"
          className="runtime-summary"
          data-session-state={sessionState}
          data-testid="runtime-status"
          data-trial-available={runtime.data?.trial_available ?? "unknown"}
        >
          <span className={`runtime-badge ${runtime.data?.trial_available ? "available" : ""}`}>
            <i />
            {runtime.data === null
              ? "トライアル確認中"
              : runtime.data.trial_available
                ? "トライアル利用可"
                : "トライアル利用不可"}
          </span>
          {runtimeSession === null ? (
            <span className={`runtime-badge session-${sessionState}`}>
              <i />
              {sessionLabel}
            </span>
          ) : (
            <Link
              className={`runtime-badge session-${sessionState}`}
              data-testid="runtime-session-link"
              href={routePath("try", runtimeSession.id)}
            >
              <i />
              {sessionLabel}
            </Link>
          )}
        </div>
      </header>

      <aside className="sidebar" aria-label="主なナビゲーション">
        <nav>
          {navigation.map((item) => (
            <Link
              aria-current={item.route === active ? "page" : undefined}
              className={item.route === active ? "nav-link active" : "nav-link"}
              href={routePath(item.route)}
              key={item.route}
            >
              <span>{item.index}</span>
              {item.label}
            </Link>
          ))}
        </nav>
      </aside>

      <main className="main-column">
        <section className="page-intro">
          <h1>{title}</h1>
          <p>{description}</p>
        </section>
        {children}
      </main>
      </div>
    </RuntimeStatusContext.Provider>
  );
}

function shortSessionId(id: string): string {
  return id.length > 8 ? id.slice(0, 8) : id;
}
