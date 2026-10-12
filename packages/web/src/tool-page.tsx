import { EngineError, outputName, withDefaults, type Options, type Tool } from "@pdfvault/engine";
import { useCallback, useEffect, useId, useRef, useState, useSyncExternalStore } from "react";
import { FieldInput, visible } from "./fields.tsx";
import { Icon } from "./icons.tsx";
import { Report, bytes } from "./reports.tsx";
import { runJob, warmUp, zipFiles } from "./runner.ts";
import { vault, type VaultFile } from "./vault.ts";

interface InFile {
  key: string;
  name: string;
  bytes: Uint8Array;
}

interface Produced {
  name: string;
  bytes: Uint8Array;
}

interface Outcome {
  input: string;
  files: Produced[];
  report: unknown;
  error?: { code: string; message: string };
}

const MIME: Record<string, string> = {
  pdf: "application/pdf",
  png: "image/png",
  jpg: "image/jpeg",
  webp: "image/webp",
  md: "text/markdown",
  txt: "text/plain",
  epub: "application/epub+zip",
  zip: "application/zip",
};

export function download(name: string, data: Uint8Array) {
  const ext = name.split(".").pop() ?? "";
  const url = URL.createObjectURL(new Blob([data as BlobPart], { type: MIME[ext] ?? "application/octet-stream" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.rel = "noopener";
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 30_000);
}

const isPdf = (b: Uint8Array) => new TextDecoder().decode(b.subarray(0, 1024)).includes("%PDF-");

/** Which drop slots a tool shows. Paired tools get two labeled slots. */
function slots(tool: Tool): { label: string; accept: string; multiple: boolean; kind: "pdf" | "image" }[] {
  const pdf = { accept: "application/pdf,.pdf", kind: "pdf" as const };
  const image = { accept: "image/jpeg,image/png,image/webp,image/gif,image/bmp", kind: "image" as const };
  switch (tool.input) {
    case "pdf":
      return [{ ...pdf, label: "PDF files", multiple: true }];
    case "pdfs":
      return [{ ...pdf, label: "PDF files, in order", multiple: true }];
    case "image":
      return [{ ...image, label: "Images", multiple: true }];
    case "images":
      return [{ ...image, label: "Images, one per page", multiple: true }];
    case "pdf+image":
      return [
        { ...pdf, label: "PDF", multiple: false },
        { ...image, label: "Signature image (PNG with transparency works best)", multiple: false },
      ];
    case "pdf+pdf":
      return [
        { ...pdf, label: "Original PDF", multiple: false },
        { ...pdf, label: "Changed PDF", multiple: false },
      ];
  }
}

function Drop({
  label,
  accept,
  multiple,
  kind,
  files,
  onFiles,
  onRemove,
  onMove,
  ordered,
}: {
  label: string;
  accept: string;
  multiple: boolean;
  kind: "pdf" | "image";
  files: InFile[];
  onFiles: (f: InFile[]) => void;
  onRemove: (key: string) => void;
  onMove: (key: string, by: -1 | 1) => void;
  ordered: boolean;
}) {
  const [over, setOver] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const id = useId();
  const take = async (list: FileList | null) => {
    if (!list?.length) return;
    const read = await Promise.all(
      [...list].map(async (f) => ({ key: crypto.randomUUID(), name: f.name, bytes: new Uint8Array(await f.arrayBuffer()) })),
    );
    const wrong = read.filter((f) => (kind === "pdf") !== isPdf(f.bytes));
    setProblem(wrong.length ? `${wrong.map((f) => f.name).join(", ")} ${wrong.length > 1 ? "are" : "is"} not ${kind === "pdf" ? "a PDF" : "an image"}.` : null);
    const ok = read.filter((f) => !wrong.includes(f));
    if (ok.length) onFiles(multiple ? ok : ok.slice(0, 1));
  };
  return (
    <div className="slot">
      <div
        className="drop"
        data-over={over}
        onDragOver={(e) => {
          e.preventDefault();
          setOver(true);
        }}
        onDragLeave={() => setOver(false)}
        onDrop={(e) => {
          e.preventDefault();
          setOver(false);
          void take(e.dataTransfer.files);
        }}
      >
        <Icon name={kind === "pdf" ? "file" : "image"} />
        <p>
          <strong>{label}</strong>
          <span>Drop {multiple ? "files" : "a file"} here or</span>
        </p>
        <button type="button" className="secondary" onClick={() => input.current?.click()} aria-describedby={id}>
          Choose {multiple ? "files" : "a file"}
        </button>
        <input
          ref={input}
          id={id}
          type="file"
          hidden
          accept={accept}
          multiple={multiple}
          aria-label={label}
          onChange={(e) => {
            void take(e.currentTarget.files);
            e.currentTarget.value = "";
          }}
        />
      </div>
      {problem ? <p className="error-text" role="alert">{problem}</p> : null}
      {files.length ? (
        <ol className="files">
          {files.map((f, i) => (
            <li key={f.key}>
              <span className="name">{f.name}</span>
              <span className="size">{bytes(f.bytes.length)}</span>
              {ordered && files.length > 1 ? (
                <>
                  <button type="button" className="icon" disabled={i === 0} onClick={() => onMove(f.key, -1)} aria-label={`Move ${f.name} up`}>
                    <Icon name="up" />
                  </button>
                  <button type="button" className="icon" disabled={i === files.length - 1} onClick={() => onMove(f.key, 1)} aria-label={`Move ${f.name} down`}>
                    <Icon name="down" />
                  </button>
                </>
              ) : null}
              <button type="button" className="icon" onClick={() => onRemove(f.key)} aria-label={`Remove ${f.name}`}>
                <Icon name="x" />
              </button>
            </li>
          ))}
        </ol>
      ) : null}
    </div>
  );
}

function VaultPicker({ onPick }: { onPick: (f: InFile) => void }) {
  const unlocked = useSyncExternalStore((fn) => vault.subscribe(fn), () => vault.unlocked);
  const [files, setFiles] = useState<VaultFile[] | null>(null);
  if (!unlocked) return null;
  return (
    <details
      className="vault-picker"
      onToggle={(e) => {
        if ((e.currentTarget as HTMLDetailsElement).open) void vault.list().then(setFiles);
      }}
    >
      <summary>
        <Icon name="lock" /> Add from your vault
      </summary>
      {files?.length ? (
        <ul>
          {files.map((f) => (
            <li key={f.id}>
              <button type="button" className="link" onClick={async () => onPick({ key: crypto.randomUUID(), name: f.name, bytes: await vault.read(f.id) })}>
                {f.name}
              </button>
              <span className="size">{bytes(f.size)}</span>
            </li>
          ))}
        </ul>
      ) : (
        <p className="note">{files ? "The vault is empty." : "Opening…"}</p>
      )}
    </details>
  );
}

export function ToolPage({ tool }: { tool: Tool }) {
  const layout = slots(tool);
  const [inputs, setInputs] = useState<InFile[][]>(() => layout.map(() => []));
  const [options, setOptions] = useState<Options>(() => withDefaults(tool, {}));
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<{ label: string; fraction: number | null } | null>(null);
  const [outcomes, setOutcomes] = useState<Outcome[]>([]);
  const [needsPassword, setNeedsPassword] = useState(false);
  const [saved, setSaved] = useState<Set<string>>(() => new Set());
  const abort = useRef<AbortController | null>(null);
  const unlocked = useSyncExternalStore((fn) => vault.subscribe(fn), () => vault.unlocked);
  const formId = useId();

  useEffect(() => {
    // Start compiling what this tool needs while the user picks files.
    warmUp("core");
    if (tool.heavy) warmUp("render");
    if (tool.heavy === "ocr") warmUp("ocr");
  }, [tool]);

  const setSlot = (i: number, fn: (f: InFile[]) => InFile[]) => setInputs((all) => all.map((s, j) => (j === i ? fn(s) : s)));
  const ready = layout.every((s, i) => inputs[i]!.length > 0) && (tool.input !== "pdfs" || inputs[0]!.length >= 2);
  const missing = tool.fields.filter((f) => "required" in f && f.required && visible(f, options) && (options[f.name] === undefined || options[f.name] === ""));

  const run = useCallback(async () => {
    const batch = tool.input === "pdf" || tool.input === "image";
    const jobs: InFile[][] = batch ? inputs[0]!.map((f) => [f]) : [inputs.flat()];
    const heavy = tool.heavy === "ocr" ? (["render", "ocr"] as const) : tool.heavy === "render" ? (["render"] as const) : ([] as const);
    abort.current = new AbortController();
    setRunning(true);
    setOutcomes([]);
    setSaved(new Set());
    setNeedsPassword(false);
    const results: Outcome[] = [];
    for (const [n, job] of jobs.entries()) {
      const label = job.map((f) => f.name).join(" + ");
      try {
        const r = await runJob(
          tool.id,
          job.map((f) => f.bytes),
          options,
          [...heavy],
          (l, fraction) => setProgress({ label: jobs.length > 1 ? `${label} — ${l}` : l, fraction: fraction ?? (jobs.length > 1 ? n / jobs.length : null) }),
          abort.current.signal,
        );
        results.push({ input: label, report: r.report, files: r.files.map((f) => ({ name: outputName(job[0]!.name, f), bytes: f.bytes })) });
      } catch (e) {
        const err = e instanceof EngineError ? e : new EngineError("internal", String(e));
        if (err.message === "cancelled") break;
        if (err.code === "password_required" || err.code === "wrong_password") setNeedsPassword(true);
        results.push({ input: label, files: [], report: null, error: { code: err.code, message: err.message } });
      }
      setOutcomes([...results]);
    }
    setRunning(false);
    setProgress(null);
  }, [tool, inputs, options]);

  const allFiles = outcomes.flatMap((o) => o.files);
  const passwordField = tool.fields.find((f) => f.name === "password");
  const main = tool.fields.filter((f) => !f.advanced && visible(f, options));
  const advanced = tool.fields.filter((f) => f.advanced && visible(f, options) && !(needsPassword && f.name === "password"));
  const set = (name: string) => (v: Options[string]) => setOptions((o) => ({ ...o, [name]: v }));

  return (
    <article className="tool">
      <header className="tool-head">
        <a href="#/" className="back">
          <Icon name="back" /> All tools
        </a>
        <h1>{tool.title}</h1>
        <p>{tool.summary}</p>
      </header>

      <section className="inputs" aria-label="Files">
        {layout.map((s, i) => (
          <Drop
            key={s.label}
            {...s}
            files={inputs[i]!}
            ordered={tool.input === "pdfs" || tool.input === "images"}
            onFiles={(f) => setSlot(i, (cur) => (s.multiple ? [...cur, ...f] : f))}
            onRemove={(k) => setSlot(i, (cur) => cur.filter((f) => f.key !== k))}
            onMove={(k, by) =>
              setSlot(i, (cur) => {
                const at = cur.findIndex((f) => f.key === k);
                const next = [...cur];
                const [item] = next.splice(at, 1);
                next.splice(at + by, 0, item!);
                return next;
              })
            }
          />
        ))}
        <VaultPicker onPick={(f) => setSlot(0, (cur) => (layout[0]!.multiple ? [...cur, f] : [f]))} />
      </section>

      <form
        id={formId}
        className="options"
        onSubmit={(e) => {
          e.preventDefault();
          if (ready && !missing.length && !running) void run();
        }}
      >
        {needsPassword && passwordField ? (
          <div className="locked" role="alert">
            <Icon name="lock" />
            <FieldInput field={{ ...passwordField, label: "This PDF is locked. Its password:" }} value={options.password} onChange={set("password")} idPrefix={formId} />
          </div>
        ) : null}
        {main.length ? <div className="grid">{main.map((f) => <FieldInput key={f.name} field={f} value={options[f.name]} onChange={set(f.name)} idPrefix={formId} />)}</div> : null}
        {advanced.length ? (
          <details className="more">
            <summary>More options</summary>
            <div className="grid">
              {advanced.map((f) => (
                <FieldInput key={f.name} field={f} value={options[f.name]} onChange={set(f.name)} idPrefix={formId} />
              ))}
            </div>
          </details>
        ) : null}
        <div className="actions">
          {running ? (
            <button type="button" className="secondary" onClick={() => abort.current?.abort()}>
              Cancel
            </button>
          ) : null}
          <button type="submit" className="primary" disabled={!ready || missing.length > 0 || running}>
            {running ? "Working…" : tool.title}
          </button>
          {!ready ? <span className="hint">{tool.input === "pdfs" ? "Add at least two PDFs." : "Add a file to start."}</span> : null}
          {ready && missing.length ? <span className="hint">Fill in: {missing.map((f) => f.label).join(", ")}.</span> : null}
        </div>
      </form>

      {progress ? (
        <div className="progress" role="status" aria-live="polite">
          <div className="track" data-indeterminate={progress.fraction === null}>
            <span ref={(el) => el?.style.setProperty("--p", `${Math.round((progress.fraction ?? 0) * 100)}%`)} />
          </div>
          <p>{progress.label}…</p>
        </div>
      ) : null}

      {outcomes.length ? (
        <section className="results" aria-label="Results">
          <h2>Result{outcomes.length > 1 ? "s" : ""}</h2>
          {allFiles.length > 1 ? (
            <button
              type="button"
              className="secondary"
              onClick={async () => download(`${tool.id}-results.zip`, await zipFiles(allFiles))}
            >
              <Icon name="download" /> Download all as ZIP
            </button>
          ) : null}
          {outcomes.map((o) => (
            <div key={o.input} className="outcome" data-error={Boolean(o.error)}>
              {outcomes.length > 1 ? <h3>{o.input}</h3> : null}
              {o.error ? <p className="error-text">{o.error.message}</p> : null}
              {o.files.length ? (
                <ul className="files out">
                  {o.files.map((f) => (
                    <li key={f.name}>
                      <span className="name">{f.name}</span>
                      <span className="size">{bytes(f.bytes.length)}</span>
                      <button type="button" className="primary small" onClick={() => download(f.name, f.bytes)}>
                        <Icon name="download" /> Download
                      </button>
                      {unlocked ? (
                        <button
                          type="button"
                          className="secondary small"
                          disabled={saved.has(f.name)}
                          onClick={() =>
                            void vault.add(f.name, f.name.split(".").pop() ?? "", f.bytes).then(() => setSaved((s) => new Set(s).add(f.name)))
                          }
                        >
                          <Icon name="lock" /> {saved.has(f.name) ? "Saved" : "Save to vault"}
                        </button>
                      ) : null}
                    </li>
                  ))}
                </ul>
              ) : null}
              <Report toolId={tool.id} report={o.report} files={o.files} />
            </div>
          ))}
        </section>
      ) : null}
    </article>
  );
}
