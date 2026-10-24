import { EngineError } from "@pdfvault/engine";
import { useEffect, useId, useRef, useState } from "react";
import { Icon } from "./icons.tsx";
import { bytes } from "./reports.tsx";
import { runJob, warmUp } from "./runner.ts";
import { Drop, type InFile } from "./tool-page.tsx";
import { workspace } from "./workspace.ts";

interface Page {
  url: string;
  width: number;
  height: number;
}

const ZOOMS = [50, 75, 100, 125, 150, 200];
const isPdf = (b: Uint8Array) => new TextDecoder().decode(b.subarray(0, 1024)).includes("%PDF-");

/** Read-only viewer: pages are rasterized by the wasm renderer, like every other tool, so nothing leaves the tab. */
export function ViewerPage() {
  // A file of yours opened from its card: shown right away, edited in place.
  const [from] = useState(() => {
    const f = workspace.viewing ? workspace.get(workspace.viewing) : undefined;
    workspace.viewing = null;
    return f && f.kind === "pdf" ? f : undefined;
  });
  const [doc, setDoc] = useState<InFile | null>(null);
  const [pages, setPages] = useState<Page[]>([]);
  const [progress, setProgress] = useState<{ label: string; fraction: number | null } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [needsPassword, setNeedsPassword] = useState(false);
  const [password, setPassword] = useState("");
  const [zoom, setZoom] = useState(100);
  const [over, setOver] = useState(false);
  const abort = useRef<AbortController | null>(null);
  const pwId = useId();

  useEffect(() => {
    warmUp("core");
    warmUp("render");
    if (from) {
      setPassword(from.password ?? "");
      void open({ key: from.key, name: from.name, bytes: from.bytes, password: from.password }, from.password);
    }
    return () => abort.current?.abort();
    // Only the file the page was opened with.
  }, []);
  useEffect(() => () => pages.forEach((p) => URL.revokeObjectURL(p.url)), [pages]);

  const open = async (file: InFile, pw?: string) => {
    abort.current?.abort();
    abort.current = new AbortController();
    setDoc(file);
    setPages([]);
    setError(null);
    setNeedsPassword(false);
    // Sharp on high-density screens without rendering huge bitmaps.
    const dpi = Math.round(96 * Math.min(window.devicePixelRatio || 1, 2));
    try {
      const r = await runJob(
        "pdf2img",
        [file.bytes],
        { format: "jpeg", dpi, quality: 85, password: pw || undefined },
        ["render"],
        (label, fraction) => setProgress({ label, fraction }),
        abort.current.signal,
      );
      const meta = (r.report as { pages: { width: number; height: number }[] }).pages;
      setPages(r.files.map((f, i) => ({ url: URL.createObjectURL(new Blob([f.bytes as BlobPart], { type: "image/jpeg" })), ...meta[i]! })));
    } catch (e) {
      const err = e instanceof EngineError ? e : new EngineError("internal", String(e));
      if (err.message === "cancelled") return;
      if (err.code === "password_required" || err.code === "wrong_password") setNeedsPassword(true);
      if (err.code !== "password_required") setError(err.message);
    }
    setProgress(null);
  };

  const dropAnywhere = async (list: FileList) => {
    const f = list[0];
    if (!f) return;
    const b = new Uint8Array(await f.arrayBuffer());
    if (!isPdf(b)) return setError(`${f.name} is not a PDF.`);
    setPassword("");
    void open({ key: crypto.randomUUID(), name: f.name, bytes: b });
  };

  return (
    <article
      className="tool viewer"
      data-over={doc ? over : undefined}
      onDragOver={(e) => {
        if (!doc) return;
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOver(false);
      }}
      onDrop={(e) => {
        if (!doc) return;
        e.preventDefault();
        setOver(false);
        void dropAnywhere(e.dataTransfer.files);
      }}
    >
      <header className="tool-head">
        <a href="#/" className="back">
          <Icon name="back" /> {from ? "Your files" : "All tools"}
        </a>
        <h1>PDF viewer</h1>
        <p>Drop a PDF to read it here. Pages are drawn in this tab; the file is never uploaded.</p>
      </header>

      {doc ? (
        <div className="viewer-bar">
          <span className="name">{doc.name}</span>
          <span className="size">
            {pages.length ? `${pages.length} page${pages.length > 1 ? "s" : ""} · ` : ""}
            {bytes(doc.bytes.length)}
          </span>
          <span className="zoom" role="group" aria-label="Zoom">
            <button type="button" className="icon" disabled={zoom <= ZOOMS[0]!} onClick={() => setZoom((z) => ZOOMS[ZOOMS.indexOf(z) - 1] ?? z)} aria-label="Zoom out">
              <Icon name="minus" />
            </button>
            <output aria-live="polite">{zoom}%</output>
            <button type="button" className="icon" disabled={zoom >= ZOOMS.at(-1)!} onClick={() => setZoom((z) => ZOOMS[ZOOMS.indexOf(z) + 1] ?? z)} aria-label="Zoom in">
              <Icon name="plus" />
            </button>
          </span>
          <button
            type="button"
            className="primary small"
            onClick={() => {
              // The editor opens files from your files: put this one there first.
              const known = workspace.get(doc.key);
              const f = known ?? workspace.add([{ name: doc.name, bytes: doc.bytes, password }])[0];
              workspace.editing = f?.key ?? null;
              location.hash = "#/edit";
            }}
          >
            <Icon name="edit" /> Edit
          </button>
          <button
            type="button"
            className="secondary small"
            onClick={() => {
              abort.current?.abort();
              setDoc(null);
              setPages([]);
              setProgress(null);
              setError(null);
              setNeedsPassword(false);
              setPassword("");
            }}
          >
            <Icon name="x" /> Close
          </button>
        </div>
      ) : (
        <section className="inputs" aria-label="Files">
          <Drop
            label="PDF file"
            accept="application/pdf,.pdf"
            multiple={false}
            kind="pdf"
            files={[]}
            onFiles={([f]) => {
              setPassword("");
              if (f) void open(f);
            }}
            onRemove={() => {}}
            onMove={() => {}}
            ordered={false}
          />
        </section>
      )}

      {needsPassword && doc ? (
        <form
          className="locked"
          onSubmit={(e) => {
            e.preventDefault();
            void open(doc, password);
          }}
        >
          <Icon name="lock" />
          <div className="field">
            <label htmlFor={pwId}>This PDF is locked. Its password:</label>
            <input id={pwId} type="password" autoComplete="off" value={password} onChange={(e) => setPassword(e.currentTarget.value)} />
          </div>
          <button type="submit" className="primary" disabled={!password}>
            Open
          </button>
        </form>
      ) : null}
      {error ? (
        <p className="error-text" role="alert">
          {error}
        </p>
      ) : null}

      {progress ? (
        <div className="progress" role="status" aria-live="polite">
          <div className="track" data-indeterminate={progress.fraction === null}>
            <span ref={(el) => el?.style.setProperty("--p", `${Math.round((progress.fraction ?? 0) * 100)}%`)} />
          </div>
          <p>{progress.label}…</p>
        </div>
      ) : null}

      {pages.length ? (
        <section className="pages" aria-label="Pages" ref={(el) => el?.style.setProperty("--zoom", `${zoom}%`)}>
          {pages.map((p, i) => (
            <img key={p.url} src={p.url} width={p.width} height={p.height} alt={`Page ${i + 1}`} loading="lazy" decoding="async" />
          ))}
        </section>
      ) : null}
      {doc && pages.length ? <p className="note viewer-hint">Drop another PDF anywhere on this page to open it instead.</p> : null}
    </article>
  );
}
