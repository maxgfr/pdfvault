import { NOTE_SIZE, TEXT_ASCENT, TEXT_LINE, type FormField } from "@pdfvault/engine";
import { useRef, type PointerEvent as ReactPointerEvent } from "react";
import { bounds, moveItem, newId, type Doc, type Item, type PageRef, type SrcPage, type StampImage, type ToolName } from "./model.ts";

export interface Style {
  color: string;
  size: number;
  bold: boolean;
  lineWidth: number;
}

interface Props {
  index: number;
  page: PageRef;
  src: SrcPage;
  scale: number;
  items: Item[];
  fields: FormField[];
  values: Record<string, string | boolean>;
  tool: ToolName;
  style: Style;
  selected: string | null;
  editing: string | null;
  images: StampImage[];
  pendingImage: number | null;
  onSelect: (id: string | null) => void;
  onEdit: (id: string | null) => void;
  onPlaced: () => void;
  onFocusPage: () => void;
  commit: (fn: (d: Doc) => Doc) => void;
  live: (fn: (d: Doc) => Doc) => void;
  checkpoint: () => void;
}

/** CSS custom properties through the CSSOM: the CSP forbids style attributes. */
export function setVars(el: HTMLElement | SVGElement | null, vars: Record<string, string | number>) {
  if (!el) return;
  for (const [k, v] of Object.entries(vars)) el.style.setProperty(`--${k}`, String(v));
}

type Drag =
  | { kind: "move"; id: string; from: { x: number; y: number }; origin: Item; moved: boolean }
  | { kind: "resize"; id: string; origin: Item }
  | { kind: "box"; id: string; from: { x: number; y: number } }
  | { kind: "ink"; id: string };

const BOX_TOOLS: ToolName[] = ["whiteout", "rect", "highlight", "redact"];
const isWhiteout = (it: Item) => it.type === "rect" && it.fill === "#ffffff" && !it.stroke;

export function PageCanvas(p: Props) {
  const svg = useRef<SVGSVGElement>(null);
  const drag = useRef<Drag | null>(null);
  const { width: W, height: H } = p.src;
  const turned = p.page.rotate % 180 !== 0;

  const toPt = (e: { clientX: number; clientY: number }) => {
    const m = svg.current?.getScreenCTM();
    if (!m) return { x: 0, y: 0 };
    const q = new DOMPoint(e.clientX, e.clientY).matrixTransform(m.inverse());
    return { x: Math.min(W, Math.max(0, q.x)), y: Math.min(H, Math.max(0, H - q.y)) };
  };

  const replace = (id: string, fn: (it: Item) => Item) => (d: Doc) => ({ ...d, items: d.items.map((it) => (it.id === id ? fn(it) : it)) });

  const down = (e: ReactPointerEvent<SVGSVGElement>) => {
    if (e.button !== 0) return;
    p.onFocusPage();
    const target = e.target as Element;
    const hit = target.closest("[data-id]")?.getAttribute("data-id") ?? null;
    const handle = target.closest("[data-handle]")?.getAttribute("data-handle") ?? null;
    const pt = toPt(e);
    const src = p.page.src;
    const capture = () => svg.current?.setPointerCapture(e.pointerId);

    if (p.tool === "select" || (p.tool === "text" && hit && p.items.find((i) => i.id === hit)?.type === "text")) {
      if (handle) {
        const origin = p.items.find((i) => i.id === handle)!;
        p.checkpoint();
        drag.current = { kind: "resize", id: handle, origin };
        capture();
      } else if (hit) {
        const origin = p.items.find((i) => i.id === hit)!;
        p.onSelect(hit);
        drag.current = { kind: "move", id: hit, from: pt, origin, moved: false };
        capture();
      } else {
        p.onSelect(null);
        p.onEdit(null);
      }
      return;
    }
    e.preventDefault();
    const id = newId();
    const s = p.style;
    if (p.tool === "text") {
      p.commit((d) => ({ ...d, items: [...d.items, { id, type: "text", page: src, x: pt.x, y: pt.y + s.size * 0.5, text: "", size: s.size, color: s.color, bold: s.bold }] }));
      p.onSelect(id);
      p.onEdit(id);
      p.onPlaced();
    } else if (p.tool === "note") {
      p.commit((d) => ({ ...d, items: [...d.items, { id, type: "note", page: src, x: pt.x, y: pt.y, text: "", color: s.color }] }));
      p.onSelect(id);
      p.onEdit(id);
      p.onPlaced();
    } else if (p.tool === "image" && p.pendingImage !== null) {
      const img = p.images[p.pendingImage];
      if (!img) return;
      const w = Math.min(160, W * 0.5);
      const h = (w * img.height) / img.width;
      p.commit((d) => ({ ...d, items: [...d.items, { id, type: "image", page: src, x: pt.x - w / 2, y: pt.y - h / 2, w, h, image: p.pendingImage! }] }));
      p.onSelect(id);
      p.onPlaced();
    } else if (BOX_TOOLS.includes(p.tool)) {
      const base = { id, page: src, x: pt.x, y: pt.y, w: 0, h: 0 };
      const item: Item =
        p.tool === "whiteout"
          ? { ...base, type: "rect", fill: "#ffffff" }
          : p.tool === "rect"
            ? { ...base, type: "rect", stroke: s.color, lineWidth: s.lineWidth }
            : p.tool === "highlight"
              ? { ...base, type: "highlight", color: s.color }
              : { ...base, type: "redact" };
      p.commit((d) => ({ ...d, items: [...d.items, item] }));
      p.onSelect(id);
      drag.current = { kind: "box", id, from: pt };
      capture();
    } else if (p.tool === "ink") {
      p.commit((d) => ({ ...d, items: [...d.items, { id, type: "ink", page: src, paths: [[pt.x, pt.y]], color: s.color, lineWidth: s.lineWidth }] }));
      drag.current = { kind: "ink", id };
      capture();
    }
  };

  const move = (e: ReactPointerEvent<SVGSVGElement>) => {
    const g = drag.current;
    if (!g) return;
    const pt = toPt(e);
    if (g.kind === "move") {
      const dx = pt.x - g.from.x;
      const dy = pt.y - g.from.y;
      if (!g.moved) {
        if (Math.hypot(dx, dy) < 1.5) return;
        g.moved = true;
        p.checkpoint();
      }
      p.live(replace(g.id, () => moveItem(g.origin, dx, dy)));
    } else if (g.kind === "box") {
      p.live(
        replace(g.id, (it) =>
          "w" in it ? { ...it, x: Math.min(g.from.x, pt.x), y: Math.min(g.from.y, pt.y), w: Math.abs(pt.x - g.from.x), h: Math.abs(pt.y - g.from.y) } : it,
        ),
      );
    } else if (g.kind === "ink") {
      p.live(
        replace(g.id, (it) => {
          if (it.type !== "ink") return it;
          const path = it.paths[0]!;
          const [lx, ly] = path.slice(-2) as [number, number];
          if (Math.hypot(pt.x - lx, pt.y - ly) < 0.75) return it;
          return { ...it, paths: [[...path, pt.x, pt.y]] };
        }),
      );
    } else if (g.kind === "resize") {
      const o = g.origin;
      const [x0, , , y1] = bounds(o);
      if (o.type === "text") {
        const [bx0, , bx1] = bounds(o);
        const k = Math.max(0.2, (pt.x - bx0) / Math.max(1, bx1 - bx0));
        p.live(replace(g.id, (it) => (it.type === "text" ? { ...it, size: Math.min(300, Math.max(4, Math.round((o.size ?? 14) * k))) } : it)));
      } else if ("w" in o) {
        const w = Math.max(4, pt.x - x0);
        const h = o.type === "image" ? (w * o.h) / o.w : Math.max(4, y1 - pt.y);
        p.live(replace(g.id, (it) => ("w" in it ? { ...it, x: x0, w, h, y: y1 - h } : it)));
      }
    }
  };

  const up = () => {
    const g = drag.current;
    drag.current = null;
    if (!g) return;
    if (g.kind === "box") {
      // A click without a drag still makes a usable box.
      p.live(
        replace(g.id, (it) => {
          if (!("w" in it) || it.w > 3 || it.h > 3) return it;
          const [w, h] = it.type === "highlight" ? [120, 14] : [140, 36];
          return { ...it, y: it.y - h / 2, w, h };
        }),
      );
      p.onPlaced();
    }
    // Ink keeps the pen: one stroke per item, draw the next one right away.
  };

  const editingItem = p.items.find((i) => i.id === p.editing);
  const pageVars = { pw: W * p.scale, ph: H * p.scale, rot: p.page.rotate, scale: p.scale };

  return (
    <div className="ed-slot" data-turned={turned} ref={(el) => setVars(el, pageVars)} data-page={p.index + 1}>
      <div className="ed-page" ref={(el) => setVars(el, pageVars)}>
        {p.src.url ? <img src={p.src.url} alt="" draggable={false} /> : <div className="ed-blank" />}
        <svg
          ref={svg}
          className="ed-svg"
          data-tool={p.tool}
          viewBox={`0 0 ${W} ${H}`}
          preserveAspectRatio="none"
          role="img"
          aria-label={`Page ${p.index + 1}`}
          onPointerDown={down}
          onPointerMove={move}
          onPointerUp={up}
          onPointerCancel={up}
          onDoubleClick={(e) => {
            const hit = (e.target as Element).closest("[data-id]")?.getAttribute("data-id");
            const it = p.items.find((i) => i.id === hit);
            if (it && (it.type === "text" || it.type === "note")) p.onEdit(it.id);
          }}
        >
          {p.items.map((it) => (
            <ItemShape key={it.id} it={it} H={H} images={p.images} selected={it.id === p.selected} editing={it.id === p.editing} />
          ))}
          {p.items
            .filter((it) => it.id === p.selected && it.id !== p.editing)
            .map((it) => {
              const [x0, y0, x1, y1] = bounds(it);
              const resizable = it.type !== "ink" && it.type !== "note";
              const r = 5 / p.scale;
              return (
                <g key="sel" className="ed-selection">
                  <rect x={x0 - 2} y={H - y1 - 2} width={x1 - x0 + 4} height={y1 - y0 + 4} />
                  {resizable ? <circle data-handle={it.id} cx={x1 + 2} cy={H - y0 + 2} r={r} /> : null}
                </g>
              );
            })}
        </svg>
        {p.fields.map((f, i) => (
          <FieldOverlay key={`${f.name}-${i}`} f={f} H={H} W={W} scale={p.scale} pageId={p.page.id} values={p.values} commit={p.commit} live={p.live} checkpoint={p.checkpoint} />
        ))}
        {editingItem && (editingItem.type === "text" || editingItem.type === "note") ? (
          <TextEditor key={editingItem.id} it={editingItem} W={W} H={H} scale={p.scale} live={p.live} commit={p.commit} onDone={() => p.onEdit(null)} />
        ) : null}
      </div>
    </div>
  );
}

function ItemShape({ it, H, images, selected, editing }: { it: Item; H: number; images: StampImage[]; selected: boolean; editing: boolean }) {
  const common = { "data-id": it.id, className: `ed-item ed-${it.type}${selected ? " is-selected" : ""}` };
  switch (it.type) {
    case "text": {
      if (editing) return null;
      const size = it.size ?? 14;
      const [x0, y0, x1, y1] = bounds(it);
      return (
        <g {...common}>
          <rect className="ed-hit" x={x0} y={H - y1} width={x1 - x0} height={y1 - y0} />
          <text x={it.x} y={H - it.y + size * TEXT_ASCENT} fontSize={size} fontWeight={it.bold ? 700 : 400} fill={it.color ?? "#000000"} xmlSpace="preserve">
            {it.text.split("\n").map((line, i) => (
              <tspan key={i} x={it.x} dy={i === 0 ? 0 : size * TEXT_LINE}>
                {line || " "}
              </tspan>
            ))}
          </text>
        </g>
      );
    }
    case "rect": {
      const [x0, y0, x1, y1] = bounds(it);
      return (
        <rect
          {...common}
          className={`${common.className}${isWhiteout(it) ? " ed-whiteout" : ""}`}
          x={x0}
          y={H - y1}
          width={x1 - x0}
          height={y1 - y0}
          fill={it.fill ?? "none"}
          stroke={it.stroke ?? "none"}
          strokeWidth={it.lineWidth ?? 1}
          opacity={it.opacity ?? 1}
        />
      );
    }
    case "highlight":
    case "redact": {
      const [x0, y0, x1, y1] = bounds(it);
      return <rect {...common} x={x0} y={H - y1} width={x1 - x0} height={y1 - y0} fill={it.type === "highlight" ? (it.color ?? "#ffe14d") : "#000000"} />;
    }
    case "ink": {
      const d = it.paths
        .map((path) => {
          const pts: string[] = [];
          for (let i = 0; i + 1 < path.length; i += 2) pts.push(`${i === 0 ? "M" : "L"}${path[i]} ${H - path[i + 1]!}`);
          // A lone point: a zero-length segment, drawn as a dot by the round cap.
          if (pts.length === 1) pts.push(`L${path[0]} ${H - path[1]!}`);
          return pts.join(" ");
        })
        .join(" ");
      return (
        <g {...common}>
          <path className="ed-hit-stroke" d={d} strokeWidth={Math.max(8, (it.lineWidth ?? 2) + 6)} />
          <path d={d} fill="none" stroke={it.color ?? "#000000"} strokeWidth={it.lineWidth ?? 2} strokeLinecap="round" strokeLinejoin="round" opacity={it.opacity ?? 1} />
        </g>
      );
    }
    case "image": {
      const img = images[it.image];
      const [x0, y0, x1, y1] = bounds(it);
      return img ? (
        <image {...common} href={img.url} x={x0} y={H - y1} width={x1 - x0} height={y1 - y0} preserveAspectRatio="none" opacity={it.opacity ?? 1} />
      ) : null;
    }
    case "note":
      return (
        <g {...common}>
          <title>{it.text || "Empty note"}</title>
          <rect x={it.x} y={H - it.y} width={NOTE_SIZE} height={NOTE_SIZE} rx={3} fill={it.color ?? "#f5c518"} className="ed-note-card" />
          <path d={`M${it.x + 4} ${H - it.y + 6}h12M${it.x + 4} ${H - it.y + 10}h12M${it.x + 4} ${H - it.y + 14}h8`} className="ed-note-lines" />
        </g>
      );
  }
}

function TextEditor({
  it,
  W,
  H,
  scale,
  live,
  commit,
  onDone,
}: {
  it: Item & { type: "text" | "note" };
  W: number;
  H: number;
  scale: number;
  live: Props["live"];
  commit: Props["commit"];
  onDone: () => void;
}) {
  const note = it.type === "note";
  const size = it.type === "text" ? (it.size ?? 14) : 12;
  const finish = (text: string) => {
    if (!text.trim()) commit((d) => ({ ...d, items: d.items.filter((x) => x.id !== it.id) }));
    onDone();
  };
  return (
    <textarea
      className={note ? "ed-text-editor ed-note-editor" : "ed-text-editor"}
      data-bold={it.type === "text" && it.bold ? "true" : undefined}
      aria-label={note ? "Comment" : "Text"}
      placeholder={note ? "Write a comment…" : "Type here"}
      value={it.text}
      // The box sits where the text will be drawn, at the page's scale.
      ref={(el) => {
        setVars(el, {
          left: `${((note ? it.x + NOTE_SIZE + 4 : it.x) / W) * 100}%`,
          top: `${((H - it.y) / H) * 100}%`,
          fs: `${size * scale}px`,
          color: it.type === "text" ? (it.color ?? "#000000") : "#16181d",
        });
        if (el && document.activeElement !== el && !el.dataset.focused) {
          el.dataset.focused = "1";
          el.focus();
        }
      }}
      rows={Math.max(1, it.text.split("\n").length)}
      onChange={(e) => {
        const text = e.currentTarget.value;
        live((d) => ({ ...d, items: d.items.map((x) => (x.id === it.id && (x.type === "text" || x.type === "note") ? { ...x, text } : x)) }));
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape" || (e.key === "Enter" && (e.metaKey || e.ctrlKey))) {
          e.preventDefault();
          e.currentTarget.blur();
        }
      }}
      onBlur={(e) => finish(e.currentTarget.value)}
    />
  );
}

function FieldOverlay({
  f,
  W,
  H,
  scale,
  pageId,
  values,
  commit,
  live,
  checkpoint,
}: {
  f: FormField;
  W: number;
  H: number;
  scale: number;
  pageId: string;
  values: Record<string, string | boolean>;
  commit: Props["commit"];
  live: Props["live"];
  checkpoint: () => void;
}) {
  const [x, y, w, h] = f.rect;
  const place = (el: HTMLElement | null) =>
    setVars(el, {
      left: `${(x / W) * 100}%`,
      top: `${((H - y - h) / H) * 100}%`,
      w: `${(w / W) * 100}%`,
      h: `${(h / H) * 100}%`,
      fs: `${Math.min(h * 0.62, 12) * scale}px`,
    });
  const v = values[f.name] ?? f.value;
  const setValue = (value: string | boolean, history: "commit" | "live") =>
    (history === "commit" ? commit : live)((d) => ({ ...d, fields: { ...d.fields, [f.name]: value } }));
  const label = `Form field ${f.name}`;
  if (f.kind === "checkbox") {
    const checked = v === true || (typeof v === "string" && v !== "" && v !== "Off");
    return <input ref={place} className="ed-field ed-check" type="checkbox" aria-label={label} checked={checked} disabled={f.readOnly} onChange={(e) => setValue(e.currentTarget.checked, "commit")} />;
  }
  if (f.kind === "radio") {
    return (
      <input
        ref={place}
        className="ed-field ed-check"
        type="radio"
        name={`${pageId}-${f.name}`}
        aria-label={`${label}: ${f.onState ?? ""}`}
        checked={v === f.onState}
        disabled={f.readOnly}
        onChange={() => setValue(f.onState ?? "", "commit")}
      />
    );
  }
  if (f.kind === "choice") {
    const options = f.options.includes(String(v)) || !v ? f.options : [String(v), ...f.options];
    return (
      <select ref={place} className="ed-field" aria-label={label} value={String(v)} disabled={f.readOnly} onChange={(e) => setValue(e.currentTarget.value, "commit")}>
        {!v ? <option value="" /> : null}
        {options.map((o) => (
          <option key={o} value={o}>
            {o}
          </option>
        ))}
      </select>
    );
  }
  const common = {
    className: "ed-field",
    "aria-label": label,
    value: String(v),
    disabled: f.readOnly,
    maxLength: f.maxLen ?? undefined,
    onFocus: checkpoint,
    onChange: (e: { currentTarget: { value: string } }) => setValue(e.currentTarget.value, "live"),
  };
  return f.multiline ? <textarea ref={place} {...common} /> : <input ref={place} type="text" {...common} />;
}
