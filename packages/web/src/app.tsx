import { toolById } from "@pdfvault/engine";
import { useEffect, useState, useSyncExternalStore } from "react";
import { About } from "./about.tsx";
import { EditorPage } from "./editor/editor-page.tsx";
import { Home } from "./home.tsx";
import { Icon } from "./icons.tsx";
import { ToolPage } from "./tool-page.tsx";
import { VaultPage } from "./vault-page.tsx";
import { ViewerPage } from "./viewer-page.tsx";
import { vault } from "./vault.ts";
import { useWorkspace } from "./workspace.ts";

function useRoute(): string {
  const get = () => location.hash.replace(/^#\/?/, "").split("?")[0] ?? "";
  const [route, setRoute] = useState(get);
  useEffect(() => {
    const on = () => {
      setRoute(get());
      window.scrollTo(0, 0);
    };
    addEventListener("hashchange", on);
    return () => removeEventListener("hashchange", on);
  }, []);
  return route;
}

export function useOffline(): { offlineReady: boolean } {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    if (!("serviceWorker" in navigator)) return;
    void navigator.serviceWorker.ready.then(() => setReady(true));
  }, []);
  return { offlineReady: ready };
}

export function App() {
  const route = useRoute();
  // The editor has its own page; the registry's "edit" tool is what it runs.
  const tool = route === "edit" ? undefined : toolById.get(route);
  const unlocked = useSyncExternalStore((fn) => vault.subscribe(fn), () => vault.unlocked);
  const { offlineReady } = useOffline();
  const files = useWorkspace();
  useEffect(() => {
    const titles: Record<string, string> = { vault: "Vault", view: "PDF viewer", edit: "Edit PDF", privacy: "Privacy" };
    document.title = tool ? `${tool.title} · pdfvault` : titles[route] ? `${titles[route]} · pdfvault` : "pdfvault: private PDF tools";
  }, [tool, route]);

  return (
    <>
      <a className="skip" href="#main">
        Skip to content
      </a>
      <header className="top">
        <a href="#/" className="brand" aria-label="pdfvault home">
          <svg viewBox="0 0 32 32" width="28" height="28" aria-hidden="true">
            <rect x="3" y="3" width="26" height="26" rx="7" className="brand-bg" />
            <circle cx="16" cy="16" r="6.5" className="brand-ring" />
            <path d="M16 9.5v3M16 19.5v3M9.5 16h3M19.5 16h3" className="brand-ring" />
          </svg>
          <span>pdfvault</span>
        </a>
        <nav aria-label="Main">
          {files.length && route !== "" ? (
            <a href="#/">
              <Icon name="file" /> <span className="nav-label">Your files</span> <span className="count">{files.length}</span>
            </a>
          ) : null}
          <a href="#/vault" aria-current={route === "vault" ? "page" : undefined}>
            <Icon name={unlocked ? "unlock" : "lock"} /> <span className="nav-label">Vault</span>
          </a>
          <a href="#/privacy" aria-current={route === "privacy" ? "page" : undefined}>
            <Icon name="shield" /> <span className="nav-label">Privacy</span>
          </a>
        </nav>
      </header>
      <main id="main" tabIndex={-1}>
        {tool ? (
          <ToolPage key={tool.id} tool={tool} />
        ) : route === "edit" ? (
          <EditorPage />
        ) : route === "vault" ? (
          <VaultPage />
        ) : route === "view" ? (
          <ViewerPage />
        ) : route === "privacy" ? (
          <About />
        ) : (
          <Home />
        )}
      </main>
      <footer className="bottom">
        <p>
          <Icon name="shield" /> Your files stay on this device.{" "}
          {offlineReady ? <span className="pill">Available offline</span> : null}
        </p>
        <p>
          Also a CLI: <code>bunx pdfvault compress file.pdf</code> ·{" "}
          <a href="https://github.com/maxgfr/pdfvault" rel="noopener noreferrer">
            Source
          </a>
        </p>
      </footer>
    </>
  );
}
