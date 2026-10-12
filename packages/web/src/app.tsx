import { toolById } from "@pdfvault/engine";
import { useEffect, useState, useSyncExternalStore } from "react";
import { About } from "./about.tsx";
import { Home } from "./home.tsx";
import { Icon } from "./icons.tsx";
import { ToolPage } from "./tool-page.tsx";
import { VaultPage } from "./vault-page.tsx";
import { vault } from "./vault.ts";

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
  const tool = toolById.get(route);
  const unlocked = useSyncExternalStore((fn) => vault.subscribe(fn), () => vault.unlocked);
  const { offlineReady } = useOffline();
  useEffect(() => {
    document.title = tool ? `${tool.title} · pdfvault` : route === "vault" ? "Vault · pdfvault" : "pdfvault: private PDF tools";
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
          <a href="#/vault" aria-current={route === "vault" ? "page" : undefined}>
            <Icon name={unlocked ? "unlock" : "lock"} /> Vault
          </a>
          <a href="#/privacy" aria-current={route === "privacy" ? "page" : undefined}>
            <Icon name="shield" /> Privacy
          </a>
        </nav>
      </header>
      <main id="main" tabIndex={-1}>
        {tool ? <ToolPage key={tool.id} tool={tool} /> : route === "vault" ? <VaultPage /> : route === "privacy" ? <About /> : <Home />}
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
