import { categories, tools } from "@pdfvault/engine";
import { useEffect, useState } from "react";
import { Icon } from "./icons.tsx";
import { FileDrop, Workspace, addFiles } from "./workspace-page.tsx";
import { filesFromDrop, useWorkspace } from "./workspace.ts";

export function Home() {
  const [q, setQ] = useState("");
  const [over, setOver] = useState(false);
  const files = useWorkspace();
  const needle = q.trim().toLowerCase();
  const match = (t: (typeof tools)[number]) =>
    !needle || `${t.title} ${t.summary} ${t.id} ${(t.aliases ?? []).join(" ")}`.toLowerCase().includes(needle);
  const groups = categories.map((c) => ({ ...c, items: tools.filter((t) => t.category === c.id && match(t)) })).filter((g) => g.items.length);

  useEffect(() => {
    // Pasting files or a screenshot anywhere outside a text field adds them.
    const paste = (e: ClipboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target?.closest("input, textarea, [contenteditable]")) return;
      const list = [...(e.clipboardData?.files ?? [])];
      if (!list.length) return;
      e.preventDefault();
      void addFiles(list);
    };
    addEventListener("paste", paste);
    return () => removeEventListener("paste", paste);
  }, []);

  const catalog = (
    <section className="catalog" aria-label="Tools">
      <div className="search">
        <Icon name="search" />
        <input type="search" placeholder={`Search ${tools.length} tools`} value={q} onChange={(e) => setQ(e.currentTarget.value)} aria-label="Search tools" />
      </div>
      {groups.map((g) => (
        <div key={g.id} className="group">
          <h2>
            <Icon name={g.id === "image" ? "image" : g.id} /> {g.title}
          </h2>
          <ul className="cards">
            {g.items.map((t) => (
              <li key={t.id}>
                <a href={`#/${t.id}`} className="card">
                  <strong>{t.title}</strong>
                  <span>{t.summary}</span>
                </a>
              </li>
            ))}
          </ul>
        </div>
      ))}
      {groups.length === 0 ? <p className="note">No tool matches “{q}”.</p> : null}
    </section>
  );

  return (
    <div
      className="home"
      data-over={files.length ? over : undefined}
      onDragOver={(e) => {
        if (!files.length || !e.dataTransfer.types.includes("Files")) return;
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOver(false);
      }}
      onDrop={(e) => {
        if (!files.length) return;
        e.preventDefault();
        setOver(false);
        void filesFromDrop(e.dataTransfer).then(addFiles);
      }}
    >
      {files.length ? (
        <Workspace />
      ) : (
        <section className="hero">
          <h1>
            PDF tools that never <em>upload</em> your files.
          </h1>
          <p className="lede">
            Compress, edit, merge, protect, OCR and convert PDFs and images. Everything runs in this tab, in WebAssembly. No
            server, no account, and it keeps working offline.
          </p>
          <ul className="promises">
            <li>
              <Icon name="shield" /> Nothing leaves your device
            </li>
            <li>
              <Icon name="offline" /> Works in airplane mode
            </li>
            <li>
              <Icon name="code" /> Open source, Rust + wasm
            </li>
          </ul>
          <FileDrop />
        </section>
      )}

      {files.length ? (
        // With files open, the tool list stays out of the way.
        <details className="catalog-fold">
          <summary>Or open a tool on its own page ({tools.length})</summary>
          {catalog}
        </details>
      ) : (
        <>
          <h2 className="catalog-title">Or start from a tool</h2>
          {catalog}
        </>
      )}
    </div>
  );
}
