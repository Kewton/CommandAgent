import type { ReactNode } from "react";

import { routePath, withBasePath, type GuiRoute } from "../lib/base-path";

const navigation: { route: GuiRoute; label: string; index: string }[] = [
  { route: "dashboard", label: "Overview", index: "01" },
  { route: "try", label: "Trial run", index: "02" },
  { route: "run", label: "Run detail", index: "03" },
  { route: "assets", label: "Assets", index: "04" },
  { route: "measurements", label: "Measures", index: "05" },
];

type ShellProps = {
  active: GuiRoute;
  eyebrow: string;
  title: string;
  description: string;
  children: ReactNode;
};

export function Shell({ active, eyebrow, title, description, children }: ShellProps) {
  return (
    <div className="app-shell">
      <header className="topbar">
        <a className="brand" href={withBasePath(routePath("dashboard"))}>
          <span className="brand-mark" aria-hidden="true">
            CA
          </span>
          <span>
            <strong>CommandAgent</strong>
            <small>Operational Observatory</small>
          </span>
        </a>
        <div className="readonly-pill">
          <span className="pulse-dot" /> CLI delegated
        </div>
      </header>

      <aside className="sidebar" aria-label="Dashboard navigation">
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
        <p className="sidebar-note">
          Existing gates
          <br />
          remain authoritative.
        </p>
      </aside>

      <main className="main-column">
        <section className="page-intro">
          <p className="eyebrow">{eyebrow}</p>
          <div>
            <h1>{title}</h1>
            <p>{description}</p>
          </div>
        </section>
        {children}
      </main>
    </div>
  );
}
