import { categories, tools } from "@pdfvault/engine";
import { useState } from "react";
import { Icon } from "./icons.tsx";

export function Home() {
  const [q, setQ] = useState("");
  const needle = q.trim().toLowerCase();
  const match = (t: (typeof tools)[number]) =>
    !needle || `${t.title} ${t.summary} ${t.id} ${(t.aliases ?? []).join(" ")}`.toLowerCase().includes(needle);
  const groups = categories.map((c) => ({ ...c, items: tools.filter((t) => t.category === c.id && match(t)) })).filter((g) => g.items.length);
  return (
    <>
      <section className="hero">
        <h1>
          PDF tools that never <em>upload</em> your files.
        </h1>
        <p className="lede">
          Compress, merge, split, protect, OCR and convert PDFs and images. Everything runs in this tab, in WebAssembly. No
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
        <a className="cta" href="#/compress">
          <Icon name="compress" /> Compress a PDF
        </a>
      </section>

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
    </>
  );
}
