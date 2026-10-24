import { EngineError, isFontFile, missingCharacters, outputName, type EditReport, type FormField, type PageObject, type Paragraph } from "@pdfvault/engine";
import { useEffect, useId, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { download, type InFile } from "../action-panel.tsx";
import { Icon } from "../icons.tsx";
import { bytes } from "../reports.tsx";
import { coreReport, runJob, warmUp } from "../runner.ts";
import { Drop } from "../tool-page.tsx";
import { vault } from "../vault.ts";
import { workspace } from "../workspace.ts";
import { canQueryLocalFonts, findInstalledFonts, findLocalFont, knownInstalledFonts, type ExtraFont, type Family } from "./fonts.ts";
import { initialDoc, pageEdits, sources, styleOf, toOps, useHistory, moveItem, type Doc, type Item, type SrcPage, type StampImage, type ToolName } from "./model.ts";
import { PageCanvas, type Style } from "./page-canvas.tsx";
import { ImagePicker } from "./signature-pad.tsx";
import { Thumbs } from "./thumbs.tsx";

interface Source extends InFile {
  /** The workspace file this came from, to put the result back in its place. */
  from?: string;
}

interface Info {
  pages: number;
  pageSizes: { width: number; height: number; rotation: number }[];
  security: { needsPassword: boolean };
}

const TOOLS: { id: ToolName; label: string; icon: string; hint: string }[] = [
  {
    id: "select",
    label: "Select",
    icon: "cursor",
    hint: "Select, move and resize what you added and the page's own images and shapes; Delete removes them. Double-click text to change it.",
  },
  {
    id: "edittext",
    label: "Edit text",
    icon: "edittext",
    hint: "Click a paragraph of the document to change it: the text reflows and keeps its fonts. Empty it to remove it. To keep the fonts exactly, the browser may ask to use the fonts installed on this device.",
  },
  { id: "text", label: "Add text", icon: "text", hint: "Click where the text should start, then type. Any language works." },
  { id: "whiteout", label: "Whiteout", icon: "whiteout", hint: "Drag over what should disappear behind white." },
  { id: "highlight", label: "Highlight", icon: "highlight", hint: "Drag over a passage. Highlights are real comments other PDF apps can edit." },
  { id: "ink", label: "Draw", icon: "pen", hint: "Draw freehand with a mouse, a finger or a pen." },
  { id: "rect", label: "Box", icon: "square", hint: "Drag to draw a rectangle outline." },
  { id: "note", label: "Note", icon: "note", hint: "Click to pin a comment. It stays editable in other PDF apps." },
  { id: "image", label: "Sign / image", icon: "signature", hint: "Click on a page to place the signature or image." },
  {
    id: "redact",
    label: "Redact",
    icon: "redact",
    hint: "Drag over what must go for good: the text, images and drawings under the box are deleted from the file, the rest of the page stays as it is.",
  },
];

const base = { size: 14, bold: false, italic: false, family: "sans" as Family, lineWidth: 2 };
const DEFAULT_STYLE: Record<string, Style> = {
  text: { ...base, color: "#111111" },
  ink: { ...base, color: "#1a2b6d" },
  rect: { ...base, color: "#d93025" },
  highlight: { ...base, color: "#ffe14d" },
  note: { ...base, color: "#f5c518" },
};

const FAMILIES: { id: Family; label: string }[] = [
  { id: "sans", label: "Sans (Arial)" },
  { id: "serif", label: "Serif (Times)" },
  { id: "mono", label: "Mono (Courier)" },
];

const ZOOMS = [50, 75, 100, 125, 150, 200];

/** Remove an item; for one of the page's own objects, mark it deleted (and a second time, restore it). */
function drop(d: Doc, id: string): Doc {
  const it = d.items.find((i) => i.id === id);
  if (it?.type === "object" && !it.delete) return { ...d, items: d.items.map((i) => (i.id === id ? { ...it, delete: true } : i)) };
  return { ...d, items: d.items.filter((i) => i.id !== id) };
}

/** How long edits settle before the engine redraws the page. */
const PREVIEW_DELAY = 350;

export function EditorPage() {
  const [file, setFile] = useState<Source | null>(() => {
    const f = workspace.editing ? workspace.get(workspace.editing) : undefined;
    workspace.editing = null;
    return f && f.kind === "pdf" ? { key: f.key, name: f.name, bytes: f.bytes, password: f.password, from: f.key } : null;
  });
  const [pages, setPages] = useState<SrcPage[] | null>(null);
  const [fields, setFields] = useState<FormField[]>([]);
  const [paragraphs, setParagraphs] = useState<Paragraph[]>([]);
  const [objects, setObjects] = useState<PageObject[]>([]);
  // The engine's rendering of pages with rewritten text or moved objects.
  const [previews, setPreviews] = useState<Record<number, { url: string; key: string; overflow: number }>>({});
  const previewJobs = useRef<Record<number, { key: string; abort: AbortController; timer: ReturnType<typeof setTimeout> }>>({});
  const [extraFonts, setExtraFonts] = useState<ExtraFont[]>([]);
  const [missing, setMissing] = useState<string | null>(null);
  const [needsPassword, setNeedsPassword] = useState(false);
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<string | null>(null);
  const [tool, setTool] = useState<ToolName>("select");
  const [styles, setStyles] = useState(DEFAULT_STYLE);
  const [selected, setSelected] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [images, setImages] = useState<StampImage[]>([]);
  const [pendingImage, setPendingImage] = useState<number | null>(null);
  const [picker, setPicker] = useState(false);
  const [zoom, setZoom] = useState(100);
  const [fit, setFit] = useState(1);
  const [current, setCurrent] = useState(0);
  const [result, setResult] = useState<{ name: string; bytes: Uint8Array; report: EditReport } | null>(null);
  const [saved, setSaved] = useState<"vault" | "files" | null>(null);
  const history = useHistory(() => initialDoc(0));
  const { doc, commit, live, checkpoint, undo, redo, reset } = history;
  const column = useRef<HTMLDivElement>(null);
  const abort = useRef<AbortController | null>(null);
  const unlocked = useSyncExternalStore((fn) => vault.subscribe(fn), () => vault.unlocked);
  const pwId = useId();

  useEffect(() => {
    warmUp("core");
    warmUp("render");
    return () => abort.current?.abort();
  }, []);
  useEffect(() => () => pages?.forEach((p) => p.url && URL.revokeObjectURL(p.url)), [pages]);
  const imageUrls = useRef<string[]>([]);
  imageUrls.current = images.map((i) => i.url);
  useEffect(() => () => imageUrls.current.forEach((u) => URL.revokeObjectURL(u)), []);

  const open = async (f: Source, pw?: string) => {
    abort.current?.abort();
    abort.current = new AbortController();
    setFile(f);
    setPages(null);
    setError(null);
    setResult(null);
    setNeedsPassword(false);
    const opts = { password: pw || f.password || undefined };
    try {
      setProgress("Reading the document");
      const info = await coreReport<Info>("info", f.bytes, opts);
      if (info.pages === 0 && info.security.needsPassword) {
        setNeedsPassword(true);
        setProgress(null);
        return;
      }
      const form = await coreReport<{ fields: FormField[] }>("form-fields", f.bytes, opts);
      // The page's own text, for Edit text, read while the pages render.
      const lines = coreReport<{ paragraphs: Paragraph[]; objects: PageObject[] }>("text-runs", f.bytes, opts).then(
        (r) => r,
        () => ({ paragraphs: [] as Paragraph[], objects: [] as PageObject[] }),
      );
      // Sharp on high-density screens without rendering huge bitmaps.
      const dpi = Math.round(96 * Math.min(window.devicePixelRatio || 1, 2));
      const r = await runJob("pdf2img", [f.bytes], { format: "jpeg", dpi, quality: 82, ...opts }, ["render"], (label, fraction) =>
        setProgress(fraction === null ? label : `${label} (${Math.round(fraction * 100)}%)`),
        abort.current.signal,
      );
      const urls = r.files.map((x) => URL.createObjectURL(new Blob([x.bytes as BlobPart], { type: "image/jpeg" })));
      const srcPages = info.pageSizes.map((s, i) => {
        const turned = s.rotation % 180 !== 0;
        return { width: turned ? s.height : s.width, height: turned ? s.width : s.height, url: urls[i] };
      });
      setFile({ ...f, password: opts.password });
      setFields(form.fields);
      const found = await lines;
      setParagraphs(found.paragraphs ?? []);
      setObjects(found.objects ?? []);
      setPreviews({});
      setPages(srcPages);
      reset(initialDoc(srcPages.length));
      setSelected(null);
      setEditing(null);
      setCurrent(0);
    } catch (e) {
      const err = e instanceof EngineError ? e : new EngineError("internal", String(e));
      if (err.message === "cancelled") return;
      if (err.code === "password_required" || err.code === "wrong_password") {
        setNeedsPassword(true);
        if (err.code === "wrong_password") setError("That password is not right.");
      } else setError(err.message);
    } finally {
      setProgress(null);
    }
  };

  useEffect(() => {
    if (file && !pages) void open(file);
    // Only the file the page was opened with.
  }, []);

  // Fit the widest page to the column; zoom scales from there.
  useEffect(() => {
    const el = column.current;
    if (!el || !pages) return;
    const all = sources(pages, doc);
    const widest = Math.max(...doc.pages.map((p) => (p.rotate % 180 ? all[p.src]?.height : all[p.src]?.width) ?? 612));
    const ro = new ResizeObserver(([entry]) => {
      const w = entry!.contentRect.width - 8;
      setFit(Math.min(1.5, Math.max(0.2, w / widest)));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [pages, doc]);

  const srcs = useMemo(() => (pages ? sources(pages, doc) : []), [pages, doc]);
  const scale = fit * (zoom / 100);
  const styleKey = tool === "whiteout" || tool === "redact" || tool === "select" || tool === "image" || tool === "edittext" ? null : tool;
  const selectedItem = doc.items.find((i) => i.id === selected) ?? null;
  const canStyle = selectedItem && (tool === "select" || (tool === "edittext" && selectedItem.type === "replace"));
  const styleTarget = canStyle ? (selectedItem.type === "rect" && selectedItem.stroke ? "rect" : selectedItem.type) : styleKey;
  const style = styles[styleTarget ?? "text"] ?? DEFAULT_STYLE.text!;
  // What the text controls show: the selected text's own values, else the tool's.
  const typed = selectedItem && (selectedItem.type === "text" || selectedItem.type === "replace") && canStyle ? selectedItem : null;
  const shown = typed
    ? { ...styleOf(typed), size: typed.size ?? typed.look?.size ?? style.size, color: typed.color ?? typed.look?.color ?? style.color }
    : style;
  const redactions = doc.items.filter((i) => i.type === "redact").length;
  const dirty = history.canUndo;

  const setStyle = (patch: Partial<Style> & { auto?: boolean }) => {
    if (!styleTarget) return;
    if (styleTarget !== "replace") setStyles((s) => ({ ...s, [styleTarget]: { ...style, ...patch } }));
    if (selectedItem && canStyle) {
      commit((d) => ({
        ...d,
        items: d.items.map((it): Item => {
          if (it.id !== selectedItem.id) return it;
          if (it.type === "text")
            return { ...it, color: patch.color ?? it.color, size: patch.size ?? it.size, bold: patch.bold ?? it.bold, italic: patch.italic ?? it.italic, font: patch.family ?? it.font };
          if (it.type === "replace")
            return {
              ...it,
              color: patch.color ?? it.color,
              size: patch.size ?? it.size,
              bold: patch.bold ?? it.bold,
              italic: patch.italic ?? it.italic,
              font: patch.auto ? "auto" : (patch.family ?? it.font),
            };
          if (it.type === "rect" && it.stroke) return { ...it, stroke: patch.color ?? it.stroke, lineWidth: patch.lineWidth ?? it.lineWidth };
          if (it.type === "ink") return { ...it, color: patch.color ?? it.color, lineWidth: patch.lineWidth ?? it.lineWidth };
          if (it.type === "highlight" || it.type === "note") return { ...it, color: patch.color ?? it.color };
          return it;
        }),
      }));
    }
  };

  const pickTool = (t: ToolName) => {
    if (t === "image") {
      setPicker(true);
      return;
    }
    setTool(t);
    setPendingImage(null);
    setEditing(null);
    if (t !== "select") setSelected(null);
  };

  // Keyboard: delete, undo/redo, nudge, escape.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t?.closest("input, textarea, select, [contenteditable], dialog")) return;
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "z") {
        e.preventDefault();
        if (e.shiftKey) redo();
        else undo();
      } else if (mod && e.key.toLowerCase() === "y") {
        e.preventDefault();
        redo();
      } else if ((e.key === "Delete" || e.key === "Backspace") && selected) {
        e.preventDefault();
        commit((d) => drop(d, selected));
        setSelected(null);
      } else if (e.key === "Escape") {
        setSelected(null);
        setTool("select");
        setPendingImage(null);
      } else if (selected && e.key.startsWith("Arrow")) {
        e.preventDefault();
        const step = e.shiftKey ? 10 : 1;
        const [dx, dy] = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] }[e.key] ?? [0, 0];
        commit((d) => ({ ...d, items: d.items.map((i) => (i.id === selected ? moveItem(i, dx!, dy!) : i)) }));
      }
    };
    addEventListener("keydown", key);
    return () => removeEventListener("keydown", key);
  }, [selected, commit, undo, redo]);

  // The exact preview: once edits on a page settle, the engine applies them and
  // renders that page; until then the editor shows its own approximation.
  useEffect(() => {
    if (!file || !pages) return;
    const jobs = previewJobs.current;
    const edited = new Set(doc.items.filter((it) => it.page < pages.length && pageEdits(doc, it.page).length).map((it) => it.page));
    setPreviews((prev) => {
      const gone = Object.keys(prev).map(Number).filter((k) => !edited.has(k));
      if (!gone.length) return prev;
      const next = { ...prev };
      for (const k of gone) {
        URL.revokeObjectURL(next[k]!.url);
        delete next[k];
      }
      return next;
    });
    const typing = doc.items.find((i) => i.id === editing);
    for (const src of edited) {
      const key = JSON.stringify(pageEdits(doc, src));
      if (previews[src]?.key === key || jobs[src]?.key === key) continue;
      jobs[src]?.abort.abort();
      if (jobs[src]) clearTimeout(jobs[src].timer);
      delete jobs[src];
      // While text on this page is being typed, wait for it to be done.
      if (typing && typing.page === src && typing.type === "replace") continue;
      const abort = new AbortController();
      const timer = setTimeout(() => {
        const dpi = Math.round(96 * Math.min(window.devicePixelRatio || 1, 2));
        const names = [...new Set(doc.items.flatMap((i) => (i.type === "replace" && i.page === src && i.look ? [i.look.fontName] : [])))];
        const installed = knownInstalledFonts(names);
        const ops = {
          items: pageEdits(doc, src),
          fallbackFonts: extraFonts.map((f, i) => ({ file: images.length + i, name: f.postscriptName })),
          substitutes: installed.map((f, i) => ({ name: f.name, file: images.length + extraFonts.length + i })),
        };
        void runJob(
          "preview:edit",
          [file.bytes, ...images.map((i) => i.bytes), ...extraFonts.map((f) => f.bytes), ...installed.map((f) => f.bytes)],
          { ops, pages: [src], dpi, password: file.password },
          ["render"],
          () => {},
          abort.signal,
        )
          .then(async (r) => {
            if (abort.signal.aborted) return;
            const url = URL.createObjectURL(new Blob([r.files[0]!.bytes as BlobPart], { type: "image/jpeg" }));
            // Decoded before it replaces the page, so the swap does not flash white.
            const decoded = new Image();
            decoded.src = url;
            await decoded.decode().catch(() => {});
            if (abort.signal.aborted) return URL.revokeObjectURL(url);
            setPreviews((prev) => {
              if (prev[src]) URL.revokeObjectURL(prev[src].url);
              return { ...prev, [src]: { url, key, overflow: (r.report as EditReport).overflow ?? 0 } };
            });
          })
          .catch(() => {
            // Cancelled, or characters no font has: the approximation stays.
          })
          .finally(() => {
            if (jobs[src]?.abort === abort) delete jobs[src];
          });
      }, PREVIEW_DELAY);
      jobs[src] = { key, abort, timer };
    }
  }, [doc, editing, file, pages, images, extraFonts, previews]);
  useEffect(
    () => () => {
      for (const j of Object.values(previewJobs.current)) {
        j.abort.abort();
        clearTimeout(j.timer);
      }
    },
    [],
  );

  useEffect(() => {
    if (!dirty || result) return;
    const warn = (e: BeforeUnloadEvent) => e.preventDefault();
    addEventListener("beforeunload", warn);
    return () => removeEventListener("beforeunload", warn);
  }, [dirty, result]);

  const save = async (fonts: ExtraFont[] = extraFonts) => {
    if (!file || !pages) return;
    setEditing(null);
    setError(null);
    setMissing(null);
    abort.current = new AbortController();
    try {
      // The installed copies of the fonts of changed lines: a line whose embedded
      // subset lacks a character is redrawn in its real font rather than a close one.
      const names = [...new Set(doc.items.flatMap((i) => (i.type === "replace" && (i.font ?? "auto") === "auto" && i.look ? [i.look.fontName] : [])))];
      const installed = names.length && canQueryLocalFonts() ? await findInstalledFonts(names) : [];
      // Fonts the user added come after the images, tried for what the others lack.
      const fallbacks = fonts.map((f, i) => ({ file: images.length + i, name: f.postscriptName }));
      const ops = toOps(doc, pages.length, fallbacks);
      if (installed.length) ops.substitutes = installed.map((f, i) => ({ name: f.name, file: images.length + fonts.length + i }));
      const r = await runJob(
        "edit",
        [file.bytes, ...images.map((i) => i.bytes), ...fonts.map((f) => f.bytes), ...installed.map((f) => f.bytes)],
        { ops: JSON.stringify(ops), password: file.password },
        redactions ? ["render"] : [],
        (label, fraction) => setProgress(fraction === null ? label : `${label} (${Math.round(fraction * 100)}%)`),
        abort.current.signal,
      );
      const out = r.files[0]!;
      setResult({ name: outputName(file.name, out), bytes: out.bytes, report: r.report as EditReport });
      setSaved(null);
    } catch (e) {
      const err = e instanceof EngineError ? e : new EngineError("internal", String(e));
      const chars = missingCharacters(err);
      if (chars !== null) setMissing(chars);
      else if (err.message !== "cancelled") setError(err.message);
    } finally {
      setProgress(null);
    }
  };

  const addFont = (f: ExtraFont) => {
    const next = [...extraFonts, f];
    setExtraFonts(next);
    void save(next);
  };

  const useDeviceFont = async () => {
    try {
      const f = await findLocalFont(missing ?? "");
      if (f) addFont(f);
      else setError("No font on this device seems to have these characters. Choose a font file instead.");
    } catch {
      setError("The browser did not give access to this device's fonts. Choose a font file instead.");
    }
  };

  const goTo = (i: number) => {
    setCurrent(i);
    column.current?.querySelector(`[data-page="${i + 1}"]`)?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  const hint = pendingImage !== null && tool === "image" ? TOOLS.find((t) => t.id === "image")!.hint : TOOLS.find((t) => t.id === tool)!.hint;

  return (
    <article className="tool editor">
      <header className="tool-head">
        <a href="#/" className="back">
          <Icon name="back" /> {file?.from ? "Your files" : "All tools"}
        </a>
        <h1>Edit PDF</h1>
        {!pages ? <p>Add text, a signature or images, fill forms, comment, redact, and reorder or turn pages. Everything happens in this tab.</p> : null}
      </header>

      {!file ? (
        <Drop
          label="PDF to edit"
          accept="application/pdf,.pdf"
          multiple={false}
          kind="pdf"
          files={[]}
          onFiles={([f]) => {
            if (f) void open(f);
          }}
          onRemove={() => {}}
          onMove={() => {}}
          ordered={false}
        />
      ) : null}

      {needsPassword && file ? (
        <form
          className="locked"
          onSubmit={(e) => {
            e.preventDefault();
            void open(file, password);
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

      {progress ? (
        <div className="progress" role="status" aria-live="polite">
          <div className="track" data-indeterminate="true">
            <span />
          </div>
          <p>{progress}…</p>
        </div>
      ) : null}
      {error ? (
        <p className="error-text" role="alert">
          {error}
        </p>
      ) : null}

      {file && pages ? (
        <>
          <div className="ed-bar" role="toolbar" aria-label="Editing tools">
            <div className="ed-tools">
              {TOOLS.map((t) => (
                <button key={t.id} type="button" className="ed-tool" aria-pressed={tool === t.id} onClick={() => pickTool(t.id)} title={t.label}>
                  <Icon name={t.icon} /> <span>{t.label}</span>
                </button>
              ))}
            </div>
            <div className="ed-style">
              {styleTarget && styleTarget !== "image" && styleTarget !== "redact" ? (
                <label className="ed-color">
                  <span>Color</span>
                  <input type="color" value={shown.color} onChange={(e) => setStyle({ color: e.currentTarget.value })} aria-label="Color" />
                </label>
              ) : null}
              {styleTarget === "text" || styleTarget === "replace" ? (
                <>
                  <label className="ed-font">
                    <span className="sr-only">Font</span>
                    <select
                      aria-label="Font"
                      value={typed?.type === "replace" && typed.font === "auto" ? "auto" : shown.family}
                      onChange={(e) => {
                        const v = e.currentTarget.value;
                        if (v === "auto") setStyle({ auto: true });
                        else setStyle({ family: v as Family });
                      }}
                    >
                      {styleTarget === "replace" ? <option value="auto">Original font{typed?.look ? ` (${typed.look.familyName || typed.look.fontName})` : ""}</option> : null}
                      {FAMILIES.map((f) => (
                        <option key={f.id} value={f.id}>
                          {f.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="ed-num">
                    <span>Size</span>
                    <input type="number" min={4} max={300} step={0.5} value={Math.round(shown.size * 10) / 10} onChange={(e) => setStyle({ size: Number(e.currentTarget.value) || 14 })} aria-label="Text size" />
                  </label>
                  <button type="button" className="ed-tool" aria-pressed={shown.bold} onClick={() => setStyle({ bold: !shown.bold })} aria-label="Bold">
                    <strong>B</strong>
                  </button>
                  <button type="button" className="ed-tool" aria-pressed={shown.italic} onClick={() => setStyle({ italic: !shown.italic })} aria-label="Italic">
                    <em>I</em>
                  </button>
                </>
              ) : null}
              {styleTarget === "ink" || (styleTarget === "rect" && tool !== "whiteout") ? (
                <label className="ed-num">
                  <span>Line</span>
                  <input type="number" min={0.5} max={20} step={0.5} value={style.lineWidth} onChange={(e) => setStyle({ lineWidth: Number(e.currentTarget.value) || 2 })} aria-label="Line width" />
                </label>
              ) : null}
              {selectedItem && canStyle ? (
                <button
                  type="button"
                  className="icon"
                  onClick={() => {
                    commit((d) => drop(d, selectedItem.id));
                    setSelected(null);
                  }}
                  aria-label="Delete the selection"
                  title="Delete"
                >
                  <Icon name="trash" />
                </button>
              ) : null}
            </div>
            <div className="ed-history">
              <button type="button" className="icon" onClick={undo} disabled={!history.canUndo} aria-label="Undo" title="Undo (Ctrl+Z)">
                <Icon name="undo" />
              </button>
              <button type="button" className="icon" onClick={redo} disabled={!history.canRedo} aria-label="Redo" title="Redo (Ctrl+Shift+Z)">
                <Icon name="redo" />
              </button>
              <span className="zoom" role="group" aria-label="Zoom">
                <button type="button" className="icon" disabled={zoom <= ZOOMS[0]!} onClick={() => setZoom((z) => ZOOMS[ZOOMS.indexOf(z) - 1] ?? z)} aria-label="Zoom out">
                  <Icon name="minus" />
                </button>
                <output aria-live="polite">{zoom}%</output>
                <button type="button" className="icon" disabled={zoom >= ZOOMS.at(-1)!} onClick={() => setZoom((z) => ZOOMS[ZOOMS.indexOf(z) + 1] ?? z)} aria-label="Zoom in">
                  <Icon name="plus" />
                </button>
              </span>
              <button type="button" className="primary ed-save" onClick={() => void save()} disabled={Boolean(progress)}>
                <Icon name="download" /> Save PDF
              </button>
            </div>
          </div>
          <p className="note ed-hint" aria-live="polite">
            {hint}
          </p>

          {Object.values(previews).some((p) => p.overflow > 0) ? (
            <p className="note ed-overflow" role="status">
              A changed paragraph is now longer than it was and runs into what follows it: nothing below moves in a PDF. Shorten it or make its text smaller.
            </p>
          ) : null}
          {missing !== null ? (
            <div className="ed-missing" role="alert">
              <p>
                No font the editor has can write {missing ? <strong>{missing}</strong> : "some of these characters"}. Add a font that has them and the PDF is saved again.
              </p>
              <div className="actions">
                {canQueryLocalFonts() ? (
                  <button type="button" className="primary small" onClick={() => void useDeviceFont()}>
                    Use a font from this device
                  </button>
                ) : null}
                <label className="secondary small file-button">
                  Choose a font file
                  <input
                    type="file"
                    accept=".ttf,.otf,.ttc,font/ttf,font/otf,font/collection"
                    aria-label="Font file"
                    onChange={(e) => {
                      const f = e.currentTarget.files?.[0];
                      e.currentTarget.value = "";
                      if (!f) return;
                      void f.arrayBuffer().then((b) => {
                        const bytes = new Uint8Array(b);
                        if (!isFontFile(bytes)) setError(`${f.name} is not a TrueType or OpenType font (.ttf, .otf, .ttc).`);
                        else addFont({ name: f.name, bytes });
                      });
                    }}
                  />
                </label>
              </div>
            </div>
          ) : null}
          {extraFonts.length ? (
            <p className="note ed-fonts">
              Added fonts:{" "}
              {extraFonts.map((f, i) => (
                <span key={i} className="chip">
                  {f.name}
                  <button type="button" className="icon" aria-label={`Remove ${f.name}`} onClick={() => setExtraFonts((all) => all.filter((_, j) => j !== i))}>
                    <Icon name="x" />
                  </button>
                </span>
              ))}
            </p>
          ) : null}

          {result ? (
            <div className="ed-result" role="status">
              <span className="name">{result.name}</span>
              <span className="size">
                {bytes(result.bytes.length)} · {result.report.pages} page{result.report.pages > 1 ? "s" : ""}
                {result.report.redactedPages.length ? ` · redacted page${result.report.redactedPages.length > 1 ? "s" : ""} ${result.report.redactedPages.join(", ")}` : ""}
                {result.report.flattenedPages.length
                  ? ` (page${result.report.flattenedPages.length > 1 ? "s" : ""} ${result.report.flattenedPages.join(", ")} turned into images to be sure)`
                  : ""}
                {result.report.covered ? ` · ${result.report.covered} line${result.report.covered > 1 ? "s" : ""} could only be covered, not removed` : ""}
                {result.report.objects ? ` · ${result.report.objects} image${result.report.objects > 1 ? "s or shapes" : " or shape"} moved or removed` : ""}
                {result.report.substituted ? ` · ${result.report.substituted} line${result.report.substituted > 1 ? "s" : ""} redrawn with the font installed on this device` : ""}
                {result.report.mixed
                  ? ` · ${result.report.mixed} line${result.report.mixed > 1 ? "s" : ""} kept their font except for characters it did not include`
                  : ""}
              </span>
              <button type="button" className="primary small" onClick={() => download(result.name, result.bytes)}>
                <Icon name="download" /> Download
              </button>
              {unlocked ? (
                <button
                  type="button"
                  className="secondary small"
                  disabled={saved === "vault"}
                  onClick={() => void vault.add(result.name, "pdf", result.bytes).then(() => setSaved("vault"))}
                >
                  <Icon name="lock" /> {saved === "vault" ? "Saved" : "Save to vault"}
                </button>
              ) : null}
              <button
                type="button"
                className="secondary small"
                onClick={() => {
                  if (file.from && workspace.get(file.from)) workspace.replace(file.from, result.name, result.bytes);
                  else workspace.add([{ name: result.name, bytes: result.bytes, unsaved: true }]);
                  location.hash = "#/";
                }}
              >
                {file.from ? "Back to your files with this version" : "Keep working on it"}
              </button>
            </div>
          ) : null}

          <div className="ed-layout">
            <Thumbs pages={doc.pages} srcs={srcs} inputPages={pages.length} current={current} onGo={goTo} commit={commit} />
            <div className="ed-column" ref={column}>
              {fields.length ? (
                <label className="check ed-flatten">
                  <input type="checkbox" checked={doc.flattenFields} onChange={(e) => commit((d) => ({ ...d, flattenFields: e.currentTarget.checked }))} />
                  <span>Lock the form after filling (flatten fields)</span>
                </label>
              ) : null}
              <section className="ed-pages" aria-label="Pages">
                {doc.pages.map((pg, i) => {
                  const src = srcs[pg.src];
                  if (!src) return null;
                  return (
                    <PageCanvas
                      key={pg.id}
                      index={i}
                      page={pg}
                      src={src}
                      scale={scale}
                      items={doc.items.filter((it) => it.page === pg.src)}
                      fields={fields.filter((f) => f.page === pg.src)}
                      paragraphs={paragraphs.filter((q) => q.page === pg.src)}
                      objects={objects.filter((o) => o.page === pg.src)}
                      preview={previews[pg.src]?.key === JSON.stringify(pageEdits(doc, pg.src)) ? previews[pg.src]!.url : undefined}
                      values={doc.fields}
                      tool={tool}
                      style={style}
                      selected={selected}
                      editing={editing}
                      images={images}
                      pendingImage={pendingImage}
                      onSelect={setSelected}
                      onEdit={setEditing}
                      onPlaced={() => {
                        setTool("select");
                        setPendingImage(null);
                      }}
                      onFocusPage={() => setCurrent(i)}
                      commit={commit}
                      live={live}
                      checkpoint={checkpoint}
                    />
                  );
                })}
              </section>
              {/* Below the pages, so nothing above them moves while you draw. */}
              {redactions ? (
                <div className="ed-redaction" aria-live="polite">
                  <p className="note">
                    {redactions} redaction{redactions > 1 ? "s" : ""}:{" "}
                    {doc.redaction === "flatten"
                      ? "those pages become images when saved."
                      : "what is under the boxes is deleted from the file; the rest of the page stays as it is, text included."}
                  </p>
                  <label className="check">
                    <input
                      type="checkbox"
                      checked={doc.redaction === "flatten"}
                      onChange={(e) => commit((d) => ({ ...d, redaction: e.currentTarget.checked ? "flatten" : "remove" }))}
                    />
                    <span>Turn redacted pages into images (strictest: nothing on them stays selectable)</span>
                  </label>
                </div>
              ) : null}
              <p className="note viewer-hint">
                Changed text keeps its own font when that font has the letters; otherwise it is written in Liberation Sans, Serif or Mono, which match
                Arial, Times and Courier. Any language works: for scripts those fonts lack, add a font from your device.
              </p>
            </div>
          </div>
        </>
      ) : null}

      <ImagePicker
        open={picker}
        onClose={() => setPicker(false)}
        onPick={(img) => {
          setImages((all) => [...all, img]);
          setPendingImage(images.length);
          setTool("image");
          setSelected(null);
        }}
      />
    </article>
  );
}
