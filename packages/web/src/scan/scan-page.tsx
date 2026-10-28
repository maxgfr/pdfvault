import { EngineError, type ModelPackId } from "@pdfvault/engine";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { ShareButton, download } from "../action-panel.tsx";
import { Icon } from "../icons.tsx";
import { ModelPrompt } from "../model-prompt.tsx";
import { bytes as size } from "../reports.tsx";
import { runJob } from "../runner.ts";
import { vault } from "../vault.ts";
import { workspace } from "../workspace.ts";
import { Camera, hasCamera } from "./camera.tsx";
import { Corners } from "./corners.tsx";
import { photoBytes, session, takeShared, useScanFilter, useScanPages, type Filter } from "./session.ts";

const LOOKS: { value: Filter; label: string }[] = [
  { value: "color", label: "Color" },
  { value: "gray", label: "Grayscale" },
  { value: "bw", label: "Black and white" },
  { value: "original", label: "Original" },
];

const coarse = () => typeof matchMedia === "function" && matchMedia("(pointer: coarse)").matches;
const shared = () => location.hash.includes("shared");

interface Made {
  key: string;
  name: string;
  bytes: Uint8Array;
}

export function ScanPage() {
  const pages = useScanPages();
  const filter = useScanFilter();
  const unlocked = useSyncExternalStore((fn) => vault.subscribe(fn), () => vault.unlocked);
  // On a phone the scanner opens on the viewfinder; on a computer it waits to be asked.
  const [camera, setCamera] = useState(() => session.pages.length === 0 && hasCamera() && coarse() && !shared());
  const [adjusting, setAdjusting] = useState<string | null>(null);
  const [pageSize, setPageSize] = useState("a4");
  const [ocr, setOcr] = useState(false);
  const [problems, setProblems] = useState<string[]>([]);
  const [progress, setProgress] = useState<{ label: string; fraction: number | null } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [needsModel, setNeedsModel] = useState<ModelPackId | null>(null);
  const [made, setMade] = useState<Made | null>(null);
  const [saved, setSaved] = useState(false);
  const abort = useRef<AbortController | null>(null);
  const picker = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // Files shared from another app arrive through the service worker.
    if (!shared()) return;
    void takeShared().then((photos) => {
      history.replaceState(null, "", "#/scan");
      if (!photos && workspace.files.length) location.hash = "#/";
    });
  }, []);

  // A page, corner or setting changed since the PDF was made makes it stale.
  const recipe = [filter, pageSize, ocr, ...pages.map((p) => `${p.key}:${p.rotate}:${p.corners?.flat().join(",")}`)].join("|");
  useEffect(() => setMade(null), [recipe]);

  const addPhotos = async (files: File[]) => {
    const found: Uint8Array[] = [];
    const bad: string[] = [];
    for (const f of files) {
      try {
        found.push(await photoBytes(f, f.name));
      } catch (e) {
        bad.push(`${f.name}: ${e instanceof Error ? e.message : String(e)}`);
      }
    }
    setProblems(bad);
    if (found.length) session.add(found);
  };

  const make = async () => {
    setError(null);
    setNeedsModel(null);
    setMade(null);
    setSaved(false);
    const ctrl = new AbortController();
    abort.current = ctrl;
    try {
      const r = await runJob(
        "scan",
        session.pages.map((p) => p.bytes),
        { filter, pageSize, ocr, corners: JSON.stringify(session.settings()) },
        ocr ? ["render", "ocr"] : [],
        (label, fraction) => setProgress({ label, fraction }),
        ctrl.signal,
      );
      const name = `scan-${new Date().toISOString().slice(0, 10)}.pdf`;
      const bytes = r.files[0]!.bytes;
      const [file] = workspace.add([{ name, bytes, unsaved: true }]);
      setMade({ key: file!.key, name, bytes });
    } catch (e) {
      if (e instanceof EngineError && e.code === "model_missing" && e.pack) setNeedsModel(e.pack);
      else if (!ctrl.signal.aborted) setError(e instanceof Error ? e.message : String(e));
    } finally {
      setProgress(null);
      abort.current = null;
    }
  };

  const editing = adjusting ? session.get(adjusting) : undefined;
  const pending = pages.some((p) => !p.corners && !p.error);
  const last = pages.at(-1);

  if (camera) {
    return (
      <Camera
        count={pages.length}
        last={last?.preview ?? last?.photo}
        onShot={(jpeg) => session.add([jpeg])}
        onPick={(files) => {
          void addPhotos(files);
          setCamera(false);
        }}
        onDone={() => setCamera(false)}
      />
    );
  }

  return (
    <article className="tool scan">
      <header className="tool-head">
        <a href="#/" className="back">
          <Icon name="back" /> All tools
        </a>
        <h1>Scan documents</h1>
        <p>
          Photograph each page: the sheet is found, straightened and cleaned up, then everything becomes one PDF. The photos never
          leave this device.
        </p>
      </header>

      {editing ? (
        <section aria-label={`Corners of page ${pages.indexOf(editing) + 1}`}>
          <h2 className="scan-sub">Page {pages.indexOf(editing) + 1}: drag the corners onto the paper</h2>
          <Corners page={editing} onClose={() => setAdjusting(null)} />
        </section>
      ) : (
        <>
          <div className="scan-add">
            {hasCamera() ? (
              <button type="button" className="primary" onClick={() => setCamera(true)}>
                <Icon name="camera" /> {pages.length ? "Add pages with the camera" : "Use the camera"}
              </button>
            ) : null}
            <button type="button" className={hasCamera() ? "secondary" : "primary"} onClick={() => picker.current?.click()}>
              <Icon name="image" /> {hasCamera() ? "Choose photos" : "Take or choose photos"}
            </button>
            <input
              ref={picker}
              type="file"
              accept="image/*"
              multiple
              hidden
              aria-label="Photos to scan"
              onChange={(e) => {
                void addPhotos([...(e.currentTarget.files ?? [])]);
                e.currentTarget.value = "";
              }}
            />
            {pages.length ? (
              <button type="button" className="link" onClick={() => session.clear()}>
                Start over
              </button>
            ) : null}
          </div>
          {problems.length ? (
            <ul className="scan-problems" role="alert">
              {problems.map((p) => (
                <li key={p}>{p}</li>
              ))}
            </ul>
          ) : null}

          {pages.length ? (
            <>
              <ol className="scan-pages" aria-label="Pages">
                {pages.map((p, i) => (
                  <li key={p.key} className="scan-card" data-error={Boolean(p.error)}>
                    <button type="button" className="scan-preview" onClick={() => setAdjusting(p.key)} aria-label={`Page ${i + 1}: adjust the corners`}>
                      {p.preview ? <img src={p.preview} alt="" /> : <span className="scan-wait">{p.error ? "Could not read this photo" : "Straightening…"}</span>}
                      <span className="scan-num">{i + 1}</span>
                    </button>
                    <div className="scan-card-tools">
                      <button type="button" className="icon" onClick={() => session.move(p.key, -1)} disabled={i === 0} aria-label={`Move page ${i + 1} earlier`}>
                        <Icon name="back" />
                      </button>
                      <button
                        type="button"
                        className="icon"
                        onClick={() => session.move(p.key, 1)}
                        disabled={i === pages.length - 1}
                        aria-label={`Move page ${i + 1} later`}
                      >
                        <Icon name="forward" />
                      </button>
                      <button type="button" className="icon" onClick={() => session.turn(p.key)} aria-label={`Turn page ${i + 1}`}>
                        <Icon name="rotate-cw" />
                      </button>
                      <button type="button" className="icon" onClick={() => session.remove(p.key)} aria-label={`Delete page ${i + 1}`}>
                        <Icon name="trash" />
                      </button>
                    </div>
                  </li>
                ))}
              </ol>
              <p className="hint">Tap a page to adjust its corners.</p>

              <section className="options" aria-label="Options">
                <fieldset className="scan-looks">
                  <legend>Look</legend>
                  <div className="ws-chips">
                    {LOOKS.map((l) => (
                      <button key={l.value} type="button" className="chip" aria-pressed={filter === l.value} onClick={() => session.setFilter(l.value)}>
                        {l.label}
                      </button>
                    ))}
                  </div>
                </fieldset>
                <div className="grid">
                  <div className="field">
                    <label htmlFor="scan-size">Page size</label>
                    <select id="scan-size" value={pageSize} onChange={(e) => setPageSize(e.currentTarget.value)}>
                      <option value="a4">A4</option>
                      <option value="letter">US Letter</option>
                      <option value="fit">Fit each photo</option>
                    </select>
                  </div>
                  <label className="check" htmlFor="scan-ocr">
                    <input id="scan-ocr" type="checkbox" checked={ocr} onChange={(e) => setOcr(e.currentTarget.checked)} />
                    <span>Make the text searchable (OCR)</span>
                    <small className="hint">Slower; 50 languages.</small>
                  </label>
                </div>
                <div className="actions">
                  <button type="button" className="primary" disabled={Boolean(progress) || pending} onClick={() => void make()}>
                    <Icon name="convert" /> Make the PDF ({pages.length} page{pages.length > 1 ? "s" : ""})
                  </button>
                  {progress ? (
                    <button type="button" className="secondary" onClick={() => abort.current?.abort()}>
                      Cancel
                    </button>
                  ) : null}
                </div>
              </section>

              {progress ? (
                <div className="progress" role="status" aria-live="polite">
                  <div className="track" data-indeterminate={progress.fraction === null}>
                    <span ref={(el) => el?.style.setProperty("--p", `${Math.round((progress.fraction ?? 0) * 100)}%`)} />
                  </div>
                  <p>{progress.label}…</p>
                </div>
              ) : null}
              {error ? <p className="error-text" role="alert">{error}</p> : null}
              {needsModel ? (
                <ModelPrompt
                  pack={needsModel}
                  onReady={() => {
                    setNeedsModel(null);
                    void make();
                  }}
                  onCancel={() => setNeedsModel(null)}
                />
              ) : null}

              {made ? (
                <section className="results" aria-label="Result">
                  <h2>Your PDF</h2>
                  <div className="outcome">
                    <ul className="files out">
                      <li>
                        <span className="name">{made.name}</span>
                        <span className="size">{size(made.bytes.length)}</span>
                        <ShareButton name={made.name} data={made.bytes} className="primary small" />
                        <button
                          type="button"
                          className="secondary small"
                          onClick={() => {
                            download(made.name, made.bytes);
                            workspace.saved(made.key);
                          }}
                        >
                          <Icon name="download" /> Download
                        </button>
                        {unlocked ? (
                          <button
                            type="button"
                            className="secondary small"
                            disabled={saved}
                            onClick={() => void vault.add(made.name, "pdf", made.bytes).then(() => setSaved(true))}
                          >
                            <Icon name="lock" /> {saved ? "Saved" : "Save to vault"}
                          </button>
                        ) : null}
                        <button
                          type="button"
                          className="secondary small"
                          onClick={() => {
                            workspace.editing = made.key;
                            location.hash = "#/edit";
                          }}
                        >
                          <Icon name="edit" /> Edit
                        </button>
                      </li>
                    </ul>
                    <p className="note">
                      It is also in <a href="#/">your files</a>, ready for other tools.{" "}
                      <button
                        type="button"
                        className="link"
                        onClick={() => {
                          session.clear();
                          setMade(null);
                          if (hasCamera() && coarse()) setCamera(true);
                        }}
                      >
                        Scan another document
                      </button>
                    </p>
                  </div>
                </section>
              ) : null}
            </>
          ) : (
            <p className="note">No pages yet. Lay the sheet on a darker surface, in good light, and take one photo per page.</p>
          )}
        </>
      )}
    </article>
  );
}
