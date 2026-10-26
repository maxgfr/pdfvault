import { EngineError, outputName, withDefaults, type Options, type Tool } from "@pdfvault/engine";
import { useCallback, useEffect, useId, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { FieldInput, visible } from "./fields.tsx";
import { Icon } from "./icons.tsx";
import { Report, bytes } from "./reports.tsx";
import { runJob, warmUp, zipFiles } from "./runner.ts";
import { vault } from "./vault.ts";

export interface InFile {
  key: string;
  name: string;
  bytes: Uint8Array;
  /** Opens this PDF; overrides the form's password field. */
  password?: string;
}

export interface Produced {
  name: string;
  bytes: Uint8Array;
}

export interface Outcome {
  toolId: string;
  input: string;
  /** Keys of the files this job read, in order. */
  inputs: string[];
  files: Produced[];
  report: unknown;
  error?: { code: string; message: string };
}

/** One tool and its input slots. Several groups run under one button (a mixed batch). */
export interface Group {
  tool: Tool;
  slots: InFile[][];
  /** Heading over this group's options when there are several groups. */
  label?: string;
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

const asFile = (name: string, data: Uint8Array) =>
  new File([data as BlobPart], name, { type: MIME[name.split(".").pop() ?? ""] ?? "application/octet-stream" });

/** Whether the system share sheet takes files (phones, some desktops). */
export function canShare(name: string, data: Uint8Array): boolean {
  return typeof navigator.canShare === "function" && navigator.canShare({ files: [asFile(name, data)] });
}

/** Hand the file to another app (Mail, Messages, Drive…). Closing the sheet is not an error. */
export async function share(name: string, data: Uint8Array) {
  try {
    await navigator.share({ files: [asFile(name, data)], title: name });
  } catch (e) {
    if (!(e instanceof DOMException && e.name === "AbortError")) throw e;
  }
}

/** A Share button, rendered only where the browser can share files. */
export function ShareButton({ name, data, className = "secondary small" }: { name: string; data: Uint8Array; className?: string }) {
  if (!canShare(name, data)) return null;
  return (
    <button type="button" className={className} onClick={() => void share(name, data)}>
      <Icon name="share" /> Share
    </button>
  );
}

/** Batch tools run once per file of their first slot; the others once over every slot. */
function jobsOf(g: Group): InFile[][] {
  return g.tool.input === "pdf" || g.tool.input === "image" ? g.slots[0]!.map((f) => [f]) : [g.slots.flat()];
}

const heavyOf = (t: Tool) => (t.heavy === "ocr" ? (["render", "ocr"] as const) : t.heavy === "render" ? (["render"] as const) : ([] as const));

interface Props {
  groups: Group[];
  /** Whether the inputs are complete; `hint` says what is missing. */
  ready: boolean;
  hint?: string;
  submitLabel?: string;
  /** Extra buttons next to each produced file. */
  fileActions?: (file: Produced, outcome: Outcome) => ReactNode;
  /** Extra controls above the results, given every outcome. */
  resultsActions?: (outcomes: Outcome[]) => ReactNode;
  onDownload?: (file: Produced, outcome: Outcome) => void;
}

/** A tool's form, the run, its progress and its results. */
export function ActionPanel({ groups, ready, hint, submitLabel, fileActions, resultsActions, onDownload }: Props) {
  const [options, setOptions] = useState<Record<string, Options>>(() => Object.fromEntries(groups.map((g) => [g.tool.id, withDefaults(g.tool, {})])));
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<{ label: string; fraction: number | null } | null>(null);
  const [outcomes, setOutcomes] = useState<Outcome[]>([]);
  const [needsPassword, setNeedsPassword] = useState(false);
  const [saved, setSaved] = useState<Set<string>>(() => new Set());
  const abort = useRef<AbortController | null>(null);
  const unlocked = useSyncExternalStore((fn) => vault.subscribe(fn), () => vault.unlocked);
  const formId = useId();
  const toolKey = groups.map((g) => g.tool.id).join("+");

  useEffect(() => {
    // Start compiling what the tools need while the user sets options.
    warmUp("core");
    for (const g of groups) {
      if (g.tool.heavy) warmUp("render");
      if (g.tool.heavy === "ocr") warmUp("ocr");
    }
    // Options and results belong to one set of tools.
    setOptions(Object.fromEntries(groups.map((g) => [g.tool.id, withDefaults(g.tool, {})])));
    setOutcomes([]);
    setNeedsPassword(false);
    return () => abort.current?.abort();
  }, [toolKey]);

  const optionsOf = (t: Tool) => options[t.id] ?? withDefaults(t, {});
  const missing = groups.flatMap((g) =>
    g.tool.fields.filter((f) => {
      const o = optionsOf(g.tool);
      return "required" in f && f.required && visible(f, o) && (o[f.name] === undefined || o[f.name] === "");
    }),
  );

  const run = useCallback(async () => {
    const all = groups.flatMap((g) => jobsOf(g).map((job) => ({ tool: g.tool, job })));
    abort.current = new AbortController();
    setRunning(true);
    setOutcomes([]);
    setSaved(new Set());
    setNeedsPassword(false);
    const results: Outcome[] = [];
    for (const [n, { tool, job }] of all.entries()) {
      const label = job.map((f) => f.name).join(" + ");
      const o = optionsOf(tool);
      const password = job.find((f) => f.password)?.password ?? o.password;
      try {
        const r = await runJob(
          tool.id,
          job.map((f) => f.bytes),
          { ...o, password },
          [...heavyOf(tool)],
          (l, fraction) => setProgress({ label: all.length > 1 ? `${label} — ${l}` : l, fraction: fraction ?? (all.length > 1 ? n / all.length : null) }),
          abort.current.signal,
        );
        results.push({
          toolId: tool.id,
          input: label,
          inputs: job.map((f) => f.key),
          report: r.report,
          files: r.files.map((f) => ({ name: outputName(job[0]!.name, f), bytes: f.bytes })),
        });
      } catch (e) {
        const err = e instanceof EngineError ? e : new EngineError("internal", String(e));
        if (err.message === "cancelled") break;
        if (err.code === "password_required" || err.code === "wrong_password") setNeedsPassword(true);
        results.push({ toolId: tool.id, input: label, inputs: job.map((f) => f.key), files: [], report: null, error: { code: err.code, message: err.message } });
      }
      setOutcomes([...results]);
    }
    setRunning(false);
    setProgress(null);
  }, [groups, options]);

  const allFiles = outcomes.flatMap((o) => o.files);
  const set = (tool: Tool, name: string) => (v: Options[string]) => setOptions((all) => ({ ...all, [tool.id]: { ...optionsOf(tool), [name]: v } }));
  const title = submitLabel ?? groups[0]?.tool.title ?? "Run";
  const zipName = groups.length === 1 ? `${groups[0]!.tool.id}-results.zip` : "pdfvault-results.zip";

  return (
    <>
      <form
        id={formId}
        className="options"
        onSubmit={(e) => {
          e.preventDefault();
          if (ready && !missing.length && !running) void run();
        }}
      >
        {groups.map((g) => {
          const o = optionsOf(g.tool);
          const passwordField = g.tool.fields.find((f) => f.name === "password");
          const main = g.tool.fields.filter((f) => !f.advanced && visible(f, o));
          const advanced = g.tool.fields.filter((f) => f.advanced && visible(f, o) && !(needsPassword && f.name === "password"));
          const prefix = groups.length > 1 ? `${formId}-${g.tool.id}` : formId;
          const body = (
            <>
              {needsPassword && passwordField ? (
                <div className="locked" role="alert">
                  <Icon name="lock" />
                  <FieldInput field={{ ...passwordField, label: "This PDF is locked. Its password:" }} value={o.password} onChange={set(g.tool, "password")} idPrefix={prefix} />
                </div>
              ) : null}
              {main.length ? (
                <div className="grid">
                  {main.map((f) => (
                    <FieldInput key={f.name} field={f} value={o[f.name]} onChange={set(g.tool, f.name)} idPrefix={prefix} />
                  ))}
                </div>
              ) : null}
              {advanced.length ? (
                <details className="more">
                  <summary>More options</summary>
                  <div className="grid">
                    {advanced.map((f) => (
                      <FieldInput key={f.name} field={f} value={o[f.name]} onChange={set(g.tool, f.name)} idPrefix={prefix} />
                    ))}
                  </div>
                </details>
              ) : null}
            </>
          );
          if (groups.length === 1) return <div key={g.tool.id}>{body}</div>;
          return (
            <fieldset key={g.tool.id} className="group-options">
              <legend>{g.label ?? g.tool.title}</legend>
              {main.length || advanced.length || needsPassword ? body : <p className="note">No options.</p>}
            </fieldset>
          );
        })}
        <div className="actions">
          {running ? (
            <button type="button" className="secondary" onClick={() => abort.current?.abort()}>
              Cancel
            </button>
          ) : null}
          <button type="submit" className="primary" disabled={!ready || missing.length > 0 || running}>
            {running ? "Working…" : title}
          </button>
          {!ready && hint ? <span className="hint">{hint}</span> : null}
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
          {allFiles.length > 1 || resultsActions ? (
            <div className="results-bar">
              {allFiles.length > 1 ? (
                <button type="button" className="secondary" onClick={async () => download(zipName, await zipFiles(allFiles))}>
                  <Icon name="download" /> Download all as ZIP
                </button>
              ) : null}
              {!running ? resultsActions?.(outcomes) : null}
            </div>
          ) : null}
          {outcomes.map((o, i) => (
            <div key={`${o.input}-${i}`} className="outcome" data-error={Boolean(o.error)}>
              {outcomes.length > 1 ? <h3>{o.input}</h3> : null}
              {o.error ? <p className="error-text">{o.error.message}</p> : null}
              {o.files.length ? (
                <ul className="files out">
                  {o.files.map((f) => (
                    <li key={f.name}>
                      <span className="name">{f.name}</span>
                      <span className="size">{bytes(f.bytes.length)}</span>
                      <button
                        type="button"
                        className="primary small"
                        onClick={() => {
                          download(f.name, f.bytes);
                          onDownload?.(f, o);
                        }}
                      >
                        <Icon name="download" /> Download
                      </button>
                      <ShareButton name={f.name} data={f.bytes} />
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
                      {fileActions?.(f, o)}
                    </li>
                  ))}
                </ul>
              ) : null}
              <Report toolId={o.toolId} report={o.report} files={o.files} />
            </div>
          ))}
        </section>
      ) : null}
    </>
  );
}
