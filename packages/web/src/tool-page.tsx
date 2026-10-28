import type { Tool } from "@pdfvault/engine";
import { useId, useRef, useState, useSyncExternalStore } from "react";
import { ActionPanel, type InFile } from "./action-panel.tsx";
import { CropPreview } from "./crop-box.tsx";
import { Icon } from "./icons.tsx";
import { bytes } from "./reports.tsx";
import { vault, type VaultFile } from "./vault.ts";
import { isImage, isPdf, workspace } from "./workspace.ts";

export { download, type InFile } from "./action-panel.tsx";

type SlotKind = "pdf" | "image" | "any";

const PDF_ACCEPT = "application/pdf,.pdf";
const IMAGE_ACCEPT = "image/jpeg,image/png,image/webp,image/gif,image/bmp";

/** Which drop slots a tool shows. Paired tools get two labeled slots. */
function slots(tool: Tool): { label: string; accept: string; multiple: boolean; kind: SlotKind }[] {
  const pdf = { accept: PDF_ACCEPT, kind: "pdf" as const };
  const image = { accept: IMAGE_ACCEPT, kind: "image" as const };
  switch (tool.input) {
    case "pdf":
      return [{ ...pdf, label: "PDF files", multiple: true }];
    case "pdfs":
      return [{ ...pdf, label: "PDF files, in order", multiple: true }];
    case "image":
      return [{ ...image, label: "Images", multiple: true }];
    case "images":
      return [{ ...image, label: "Images, one per page", multiple: true }];
    case "mixed":
      return [{ accept: `${PDF_ACCEPT},${IMAGE_ACCEPT}`, kind: "any", label: "PDFs and images, in order", multiple: true }];
    case "pdf+image":
      return [
        { ...pdf, label: "PDF", multiple: false },
        { ...image, label: "Signature image (PNG with transparency works best)", multiple: false },
      ];
    case "pdf+images":
      return [
        { ...pdf, label: "PDF", multiple: false },
        { ...image, label: "Images the edits place", multiple: true },
      ];
    case "pdf+pdf":
      return [
        { ...pdf, label: "Original PDF", multiple: false },
        { ...pdf, label: "Changed PDF", multiple: false },
      ];
  }
}

const fits = (kind: SlotKind, b: Uint8Array) => (kind === "pdf" ? isPdf(b) : kind === "image" ? isImage(b) : isPdf(b) || isImage(b));
const kindName: Record<SlotKind, string> = { pdf: "a PDF", image: "an image", any: "a PDF or an image" };

export function Drop({
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
  kind: SlotKind;
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
    const wrong = read.filter((f) => !fits(kind, f.bytes));
    setProblem(wrong.length ? `${wrong.map((f) => f.name).join(", ")} ${wrong.length > 1 ? "are" : "is"} not ${kindName[kind]}.` : null);
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
          e.stopPropagation();
          setOver(false);
          void take(e.dataTransfer.files);
        }}
      >
        <Icon name={kind === "image" ? "image" : "file"} />
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

export function VaultPicker({ onPick }: { onPick: (f: InFile) => void }) {
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
  const [kept, setKept] = useState<Set<string>>(() => new Set());

  const setSlot = (i: number, fn: (f: InFile[]) => InFile[]) => setInputs((all) => all.map((s, j) => (j === i ? fn(s) : s)));
  const ready =
    layout.every((s, i) => inputs[i]!.length > 0 || (tool.input === "pdf+images" && i > 0)) && (tool.input !== "pdfs" || inputs[0]!.length >= 2);

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
            ordered={tool.input === "pdfs" || tool.input === "images" || tool.input === "mixed"}
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

      <ActionPanel
        groups={[{ tool, slots: inputs }]}
        ready={ready}
        hint={tool.input === "pdfs" ? "Add at least two PDFs." : "Add a file to start."}
        preview={
          (tool.id === "crop" || tool.id === "img-edit") && inputs[0]![0]
            ? (o, change) => <CropPreview toolId={tool.id} file={inputs[0]![0]!} options={o} onChange={change} />
            : undefined
        }
        fileActions={(f) => (
          <button
            type="button"
            className="secondary small"
            disabled={kept.has(f.name)}
            onClick={() => {
              workspace.add([{ name: f.name, bytes: f.bytes, unsaved: true }]);
              setKept((s) => new Set(s).add(f.name));
            }}
          >
            <Icon name="plus" /> {kept.has(f.name) ? "In your files" : "Keep working on it"}
          </button>
        )}
      />
    </article>
  );
}
