import { useEffect, useId, useRef, useState } from "react";
import { ActionPanel, download, type Outcome, type Produced } from "./action-panel.tsx";
import { moreActions, quickActions, type WorkAction } from "./actions.ts";
import { Icon } from "./icons.tsx";
import { bytes } from "./reports.tsx";
import { VaultPicker } from "./tool-page.tsx";
import { filesFromDrop, readFiles, useWorkspace, workspace, type WorkFile } from "./workspace.ts";

/** Add whatever the user drops, picks or pastes. */
export async function addFiles(list: Iterable<File>) {
  const read = await readFiles(list);
  if (read.length) workspace.add(read);
}

/** The home page's big drop target, shown while no file is open. */
export function FileDrop() {
  const [over, setOver] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const id = useId();
  return (
    <div
      className="hero-drop"
      data-over={over}
      onDragOver={(e) => {
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOver(false);
      }}
      onDrop={(e) => {
        e.preventDefault();
        e.stopPropagation();
        setOver(false);
        void filesFromDrop(e.dataTransfer).then(addFiles);
      }}
    >
      <Icon name="upload" />
      <p>
        <strong>Drop PDFs, photos or any file here</strong>
        <span>Several at once, or a whole folder. You can also paste a screenshot.</span>
      </p>
      <button type="button" className="primary" onClick={() => input.current?.click()} aria-describedby={id}>
        Choose files
      </button>
      <input
        ref={input}
        id={id}
        type="file"
        hidden
        multiple
        aria-label="Files to work on"
        onChange={(e) => {
          void addFiles(e.currentTarget.files ?? []);
          e.currentTarget.value = "";
        }}
      />
      <VaultPicker onPick={(f) => workspace.add([f])} />
    </div>
  );
}

function describe(f: WorkFile): string {
  const kind = f.kind === "pdf" ? `PDF${f.pages ? ` · ${f.pages} page${f.pages > 1 ? "s" : ""}` : ""}` : f.kind === "image" ? "Image" : "Not supported";
  return `${kind} · ${bytes(f.bytes.length)}`;
}

function Unlock({ file }: { file: WorkFile }) {
  const [pw, setPw] = useState("");
  const [wrong, setWrong] = useState(false);
  const id = useId();
  return (
    <form
      className="ws-unlock"
      onSubmit={async (e) => {
        e.preventDefault();
        setWrong(!(await workspace.unlock(file.key, pw)));
      }}
    >
      <label htmlFor={id}>Password</label>
      <input id={id} type="password" autoComplete="off" value={pw} onChange={(e) => setPw(e.currentTarget.value)} aria-invalid={wrong} />
      <button type="submit" className="secondary small" disabled={!pw}>
        Unlock
      </button>
      {wrong ? <span className="error-text">Wrong password.</span> : null}
    </form>
  );
}

/** Open a PDF of your files in the viewer. */
function view(f: WorkFile) {
  workspace.viewing = f.key;
  location.hash = "#/view";
}

function FileCard({ f, i, count, selected, onToggle }: { f: WorkFile; i: number; count: number; selected: boolean; onToggle: () => void }) {
  const id = useId();
  const readable = f.kind === "pdf" && !f.locked;
  const thumb = f.thumb ? <img src={f.thumb} alt="" decoding="async" /> : <Icon name={f.kind === "image" ? "image" : "file"} />;
  return (
    <li className="ws-file" data-selected={selected} data-kind={f.kind}>
      <input id={id} type="checkbox" checked={selected} disabled={f.kind === "unsupported"} onChange={onToggle} aria-label={`Select ${f.name}`} />
      {readable ? (
        <button type="button" className="ws-thumb" onClick={() => view(f)} aria-label={`Open ${f.name}`} title="Open">
          {thumb}
        </button>
      ) : (
        <label htmlFor={id} className="ws-thumb">
          {thumb}
        </label>
      )}
      <div className="ws-meta">
        <span className="name" title={f.name}>
          {f.name}
        </span>
        <span className="size">
          {describe(f)}
          {f.locked ? (
            <span className="badge">
              <Icon name="lock" /> Locked
            </span>
          ) : null}
          {f.previous ? <span className="badge">Changed</span> : null}
        </span>
        {f.problem ? <span className="ws-problem">{f.problem}</span> : null}
        {f.locked ? <Unlock file={f} /> : null}
      </div>
      <div className="ws-tools">
        {readable ? (
          <button type="button" className="icon" onClick={() => view(f)} aria-label={`View ${f.name}`} title="View">
            <Icon name="view" />
          </button>
        ) : null}
        {f.previous ? (
          <button type="button" className="link small-link" onClick={() => workspace.restore(f.key)}>
            Restore original
          </button>
        ) : null}
        {f.unsaved ? (
          <button
            type="button"
            className="icon"
            onClick={() => {
              download(f.name, f.bytes);
              workspace.saved(f.key);
            }}
            aria-label={`Download ${f.name}`}
          >
            <Icon name="download" />
          </button>
        ) : null}
        {count > 1 ? (
          <>
            <button type="button" className="icon" disabled={i === 0} onClick={() => workspace.move(f.key, -1)} aria-label={`Move ${f.name} up`}>
              <Icon name="up" />
            </button>
            <button type="button" className="icon" disabled={i === count - 1} onClick={() => workspace.move(f.key, 1)} aria-label={`Move ${f.name} down`}>
              <Icon name="down" />
            </button>
          </>
        ) : null}
        <button type="button" className="icon" onClick={() => workspace.remove(f.key)} aria-label={`Remove ${f.name}`}>
          <Icon name="x" />
        </button>
      </div>
    </li>
  );
}

/** Put a result back into the files: in place of its input when it maps one to one. */
function keep(file: Produced, o: Outcome): "replaced" | "added" {
  if (o.inputs.length === 1 && o.files.length === 1 && workspace.get(o.inputs[0]!)) {
    workspace.replace(o.inputs[0]!, file.name, file.bytes);
    return "replaced";
  }
  workspace.add([{ name: file.name, bytes: file.bytes, unsaved: true }]);
  return "added";
}

export function Workspace() {
  const files = useWorkspace();
  const [off, setOff] = useState<Set<string>>(() => new Set());
  const [actionId, setActionId] = useState<string | null>(null);
  const [kept, setKept] = useState<Map<string, "replaced" | "added">>(() => new Map());
  const input = useRef<HTMLInputElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const sel = files.filter((f) => !off.has(f.key) && f.kind !== "unsupported" && !f.locked);
  const quick = quickActions(sel);
  const more = moreActions(sel, quick);
  const all = [...quick, ...more.flatMap((c) => c.actions)];
  const action = all.find((a) => a.id === actionId) ?? null;
  const total = files.reduce((n, f) => n + f.bytes.length, 0);
  const unsaved = files.some((f) => f.unsaved);

  useEffect(() => {
    if (!unsaved) return;
    const warn = (e: BeforeUnloadEvent) => e.preventDefault();
    addEventListener("beforeunload", warn);
    return () => removeEventListener("beforeunload", warn);
  }, [unsaved]);

  const choose = (a: WorkAction) => {
    if ("route" in a.plan) {
      if (a.plan.route === "#/view") workspace.viewing = a.plan.file;
      else workspace.editing = a.plan.file;
      location.hash = a.plan.route;
      return;
    }
    setActionId(a.id);
    setKept(new Map());
    requestAnimationFrame(() => panel.current?.scrollIntoView({ behavior: "smooth", block: "start" }));
  };
  const toggleAll = () => setOff(sel.length ? new Set(files.map((f) => f.key)) : new Set());

  return (
    <section className="workspace" aria-labelledby="ws-title">
      <div className="ws-head">
        <h1 id="ws-title">Your files</h1>
        <span className="size">
          {files.length} file{files.length > 1 ? "s" : ""} · {bytes(total)} · never uploaded
        </span>
        <div className="ws-head-actions">
          <button type="button" className="secondary small" onClick={() => input.current?.click()}>
            <Icon name="plus" /> Add files
          </button>
          <button
            type="button"
            className="link small-link"
            onClick={() => {
              workspace.clear();
              setActionId(null);
            }}
          >
            Clear all
          </button>
          <input
            ref={input}
            type="file"
            hidden
            multiple
            aria-label="Add files"
            onChange={(e) => {
              void addFiles(e.currentTarget.files ?? []);
              e.currentTarget.value = "";
            }}
          />
        </div>
      </div>

      <ol className="ws-files">
        {files.map((f, i) => (
          <FileCard
            key={f.key}
            f={f}
            i={i}
            count={files.length}
            selected={!off.has(f.key) && f.kind !== "unsupported"}
            onToggle={() =>
              setOff((s) => {
                const n = new Set(s);
                if (n.has(f.key)) n.delete(f.key);
                else n.add(f.key);
                return n;
              })
            }
          />
        ))}
      </ol>
      <p className="note ws-drop-hint">Drop more files anywhere on this page, or paste them.</p>

      <div className="ws-actions">
        <div className="ws-actions-head">
          <h2>
            {sel.length ? `What to do with ${sel.length === files.length && sel.length > 1 ? "all " : ""}${sel.length} file${sel.length > 1 ? "s" : ""}?` : "Select files to see what you can do"}
          </h2>
          {files.length > 1 ? (
            <button type="button" className="link small-link" onClick={toggleAll}>
              {sel.length ? "Select none" : "Select all"}
            </button>
          ) : null}
        </div>
        {quick.length ? (
          <div className="ws-quick">
            {quick.map((a) => (
              <button key={a.id} type="button" className="ws-action" aria-pressed={a.id === actionId} onClick={() => choose(a)}>
                <Icon name={a.icon} /> {a.label}
              </button>
            ))}
          </div>
        ) : null}
        {more.length ? (
          <details className="more ws-more">
            <summary>All actions for this selection</summary>
            {more.map((c) => (
              <div key={c.id} className="ws-more-group">
                <h3>{c.title}</h3>
                <div className="ws-chips">
                  {c.actions.map((a) => (
                    <button key={a.id} type="button" className="chip" aria-pressed={a.id === actionId} onClick={() => choose(a)}>
                      {a.label}
                    </button>
                  ))}
                </div>
              </div>
            ))}
          </details>
        ) : null}
        {sel.length && !quick.length && !more.length ? <p className="note">No tool takes this mix of files. Try selecting fewer.</p> : null}
      </div>

      {action && "groups" in action.plan ? (
        <div className="ws-panel" ref={panel}>
          <h2>{action.label}</h2>
          <ActionPanel
            key={action.id}
            groups={action.plan.groups}
            ready={sel.length > 0}
            hint="Select at least one file."
            submitLabel={action.label}
            fileActions={(f, o) => {
              const done = kept.get(`${o.input}/${f.name}`);
              return (
                <button
                  type="button"
                  className="secondary small"
                  disabled={Boolean(done)}
                  onClick={() => setKept((m) => new Map(m).set(`${o.input}/${f.name}`, keep(f, o)))}
                >
                  {done === "replaced" ? "Now in your files" : done === "added" ? "Added to your files" : o.inputs.length === 1 && o.files.length === 1 ? "Use this version" : "Add to your files"}
                </button>
              );
            }}
            resultsActions={(outcomes) =>
              outcomes.filter((o) => o.files.length).length > 1 ? (
                <button
                  type="button"
                  className="primary"
                  onClick={() => {
                    const m = new Map(kept);
                    for (const o of outcomes) for (const f of o.files) if (!m.has(`${o.input}/${f.name}`)) m.set(`${o.input}/${f.name}`, keep(f, o));
                    setKept(m);
                  }}
                >
                  Keep all results in your files
                </button>
              ) : null
            }
          />
        </div>
      ) : null}
    </section>
  );
}
