"use client";

import type { ReactNode } from "react";

import { routePath, withBasePath, type GuiRoute } from "../lib/base-path";
import { useRuntimeStatus } from "../lib/use-runtime-status";

const navigation: { route: GuiRoute; label: string; index: string }[] = [
  { route: "dashboard", label: "概要", index: "01" },
  { route: "try", label: "トライアル", index: "02" },
  { route: "run", label: "実行詳細", index: "03" },
  { route: "measurements", label: "計測", index: "04" },
];

type ShellProps = {
  active: GuiRoute;
  title: string;
  description: string;
  children: ReactNode;
};

export function Shell({ active, title, description, children }: ShellProps) {
  const runtime = useRuntimeStatus();
  const sessionState = runtime.data?.session?.state ?? "idle";

  return (
    <div className="app-shell">
      <header className="topbar">
        <a className="brand" href={withBasePath(routePath("dashboard"))}>
          <span className="brand-mark" aria-hidden="true">
            CA
          </span>
          <span>
            <strong>CommandAgent</strong>
            <small>運用オブザーバトリ</small>
          </span>
        </a>
        <div
          className="runtime-summary"
          data-session-state={sessionState}
          data-testid="runtime-status"
          data-trial-available={runtime.data?.trial_available ?? "unknown"}
        >
          <span className={`runtime-badge ${runtime.data?.trial_available ? "available" : ""}`}>
            <i />
            {runtime.data === null
              ? "Trial 確認中"
              : runtime.data.trial_available
                ? "Trial 利用可"
                : "Trial 利用不可"}
          </span>
          <span className={`runtime-badge session-${sessionState}`}>
            <i />
            {runtime.failed
              ? "状態取得失敗"
              : runtime.data?.session?.state === "running"
                ? `実行中 ${shortSessionId(runtime.data.session.id)}`
                : runtime.data?.session?.state === "recovery_required"
                  ? `要復旧 ${shortSessionId(runtime.data.session.id)}`
                  : "実行中なし"}
          </span>
        </div>
      </header>

      <aside className="sidebar" aria-label="ダッシュボードのナビゲーション">
        <nav>
          {navigation.map((item) => (
            <a
              className={item.route === active ? "nav-link active" : "nav-link"}
              href={withBasePath(routePath(item.route))}
              key={item.route}
            >
              <span>{item.index}</span>
              {item.label}
            </a>
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
  );
}

function shortSessionId(id: string): string {
  return id.length > 8 ? id.slice(0, 8) : id;
}
