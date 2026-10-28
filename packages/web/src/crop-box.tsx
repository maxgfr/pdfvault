import type { Options } from "@pdfvault/engine";
import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import type { InFile } from "./action-panel.tsx";
import { coreReport, runJob } from "./runner.ts";

/** A rectangle in the picture's own units (PDF points or image pixels), from the top-left. */
interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

const CORNERS = ["Top-left", "Top-right", "Bottom-right", "Bottom-left"];
/** Handle radius on screen, in CSS pixels. */
const HANDLE = 10;
const SIDES = ["top", "right", "bottom", "left"] as const;

/** Drag a box, or its corners, over a picture of `width` × `height` units. */
function CropBox({ src, width: W, height: H, rect, onChange }: { src: string; width: number; height: number; rect: Rect; onChange: (r: Rect) => void }) {
  const svg = useRef<SVGSVGElement>(null);
  const [drag, setDrag] = useState<{ part: number; from: [number, number]; origin: Rect } | null>(null);
  const [shown, setShown] = useState(1);

  useEffect(() => {
    const el = svg.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setShown(el.getBoundingClientRect().width || 1));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const k = W / shown;
  const toUnits = (e: PointerEvent): [number, number] => {
    const r = svg.current!.getBoundingClientRect();
    return [((e.clientX - r.left) / r.width) * W, ((e.clientY - r.top) / r.height) * H];
  };
  // Part 0-3 moves that corner, 4 moves the whole box; the result stays on the picture.
  const moved = (o: Rect, part: number, dx: number, dy: number): Rect => {
    if (part === 4) {
      const x = Math.min(W - o.w, Math.max(0, o.x + dx));
      const y = Math.min(H - o.h, Math.max(0, o.y + dy));
      return { ...o, x: Math.round(x), y: Math.round(y) };
    }
    let [x0, y0, x1, y1] = [o.x, o.y, o.x + o.w, o.y + o.h];
    if (part === 0 || part === 3) x0 = Math.min(x1 - 1, Math.max(0, x0 + dx));
    else x1 = Math.max(x0 + 1, Math.min(W, x1 + dx));
    if (part < 2) y0 = Math.min(y1 - 1, Math.max(0, y0 + dy));
    else y1 = Math.max(y0 + 1, Math.min(H, y1 + dy));
    [x0, y0, x1, y1] = [x0, y0, x1, y1].map(Math.round) as [number, number, number, number];
    return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
  };
  const nudge = (part: number, e: KeyboardEvent) => {
    const step = e.shiftKey ? 10 : 1;
    const d = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[e.key];
    if (!d) return;
    e.preventDefault();
    onChange(moved(rect, part, d[0]!, d[1]!));
  };
  const grab = (part: number) => (e: PointerEvent) => {
    e.preventDefault();
    svg.current?.setPointerCapture(e.pointerId);
    setDrag({ part, from: toUnits(e), origin: rect });
  };
  const corners: [number, number][] = [
    [rect.x, rect.y],
    [rect.x + rect.w, rect.y],
    [rect.x + rect.w, rect.y + rect.h],
    [rect.x, rect.y + rect.h],
  ];

  return (
    <div className="crop-stage">
      <img src={src} alt="" draggable={false} />
      <svg
        ref={svg}
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        onPointerMove={(e) => {
          if (!drag) return;
          const [x, y] = toUnits(e);
          onChange(moved(drag.origin, drag.part, x - drag.from[0], y - drag.from[1]));
        }}
        onPointerUp={() => setDrag(null)}
        onPointerCancel={() => setDrag(null)}
      >
        <path className="crop-shade" fillRule="evenodd" d={`M0 0H${W}V${H}H0Z M${rect.x} ${rect.y}h${rect.w}v${rect.h}h${-rect.w}Z`} />
        <rect
          className="crop-edge crop-move"
          x={rect.x}
          y={rect.y}
          width={rect.w}
          height={rect.h}
          strokeWidth={2 * k}
          tabIndex={0}
          role="button"
          aria-label="Crop box. Arrow keys move it, Shift for bigger steps."
          data-active={drag?.part === 4}
          onPointerDown={grab(4)}
          onKeyDown={(e) => nudge(4, e)}
        />
        {corners.map((p, i) => (
          <circle
            key={CORNERS[i]}
            className="crop-handle"
            cx={p[0]}
            cy={p[1]}
            r={HANDLE * k}
            strokeWidth={2 * k}
            tabIndex={0}
            role="button"
            aria-label={`${CORNERS[i]} corner of the crop box. Arrow keys move it, Shift for bigger steps.`}
            data-active={drag?.part === i}
            onPointerDown={grab(i)}
            onKeyDown={(e) => nudge(i, e)}
          />
        ))}
      </svg>
    </div>
  );
}

/** The first page a page selection names ("3-5,9" → 3, "last" → count), 1-based. */
function firstPage(spec: unknown, count: number): number {
  const s = String(spec ?? "").trim();
  if (/^(last|z)\b/.test(s)) return count;
  const n = Number(/^\d+/.exec(s)?.[0] ?? 1);
  return Math.min(count, Math.max(1, n));
}

interface Picture {
  url: string;
  width: number;
  height: number;
  /** Quarter turns the page is shown with (/Rotate), 0 for images. */
  turns: number;
  /** The PDF page shown, 1-based. */
  page?: number;
}

/**
 * The crop box over the first file: page margins for Crop pages, a pixel
 * rectangle for Crop & rotate images. It writes the same numeric fields the
 * form shows, so the fields stay the precise way to set it, and the CLI's.
 */
export function CropPreview({ toolId, file, options, onChange }: { toolId: string; file: InFile; options: Options; onChange: (patch: Options) => void }) {
  const [pic, setPic] = useState<Picture | null>(null);
  const pdf = toolId === "crop";
  const page = pdf ? String(options.pages ?? "") : "";
  const password = pdf ? ((file.password ?? options.password) as string | undefined) : undefined;

  useEffect(() => {
    let url = "";
    let live = true;
    const abort = new AbortController();
    const show = (bytes: Uint8Array, type: string, size?: Omit<Picture, "url">) => {
      url = URL.createObjectURL(new Blob([bytes as BlobPart], { type }));
      if (size) return setPic({ url, ...size });
      // Browsers turn a photo by its EXIF orientation, as the engine does before cropping.
      const img = new Image();
      img.onload = () => live && setPic({ url, width: img.naturalWidth, height: img.naturalHeight, turns: 0 });
      img.src = url;
    };
    void (async () => {
      if (!pdf) return show(file.bytes, "");
      const info = await coreReport<{ pageSizes: { width: number; height: number; rotation: number }[] }>("info", file.bytes, { password });
      const n = firstPage(page, info.pageSizes.length);
      const size = info.pageSizes[n - 1];
      if (!size) return;
      const dpi = Math.round(96 * Math.min(window.devicePixelRatio || 1, 2));
      const r = await runJob("pdf2img", [file.bytes], { format: "jpeg", dpi, quality: 80, pages: String(n), password }, ["render"], () => {}, abort.signal);
      const turns = (size.rotation / 90) % 4;
      const [width, height] = turns % 2 ? [size.height, size.width] : [size.width, size.height];
      if (live && r.files[0]) show(r.files[0].bytes, "image/jpeg", { width, height, turns, page: n });
    })()
      // A locked or damaged PDF has no preview; running the tool says why.
      .catch(() => {});
    return () => {
      live = false;
      abort.abort();
      setPic(null);
      if (url) URL.revokeObjectURL(url);
    };
  }, [file.bytes, pdf, page, password]);

  if (!pic) return null;
  const { width: W, height: H, turns } = pic;
  const n = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : 0);
  let rect: Rect;
  if (pdf) {
    // Margins are cut from the unrotated page: on screen, its top is on the right after a quarter turn.
    const d = SIDES.map((_, j) => n(options[SIDES[(j - turns + 4) % 4]!]));
    rect = { x: d[3]!, y: d[0]!, w: Math.max(1, W - d[3]! - d[1]!), h: Math.max(1, H - d[0]! - d[2]!) };
  } else {
    const set = options.cropWidth !== undefined && options.cropHeight !== undefined;
    rect = set ? { x: n(options.cropX), y: n(options.cropY), w: n(options.cropWidth), h: n(options.cropHeight) } : { x: 0, y: 0, w: W, h: H };
  }
  const change = (r: Rect) => {
    if (!pdf) return onChange({ cropX: r.x, cropY: r.y, cropWidth: r.w, cropHeight: r.h });
    const d = [r.y, W - r.x - r.w, H - r.y - r.h, r.x];
    onChange(Object.fromEntries(SIDES.map((s, i) => [s, Math.max(0, Math.round(d[(i + turns) % 4]!))])));
  };

  return (
    <figure className="crop crop-tool">
      <CropBox src={pic.url} width={W} height={H} rect={rect} onChange={change} />
      <figcaption className="note">
        {pdf
          ? `Page ${pic.page}. Drag the box or its corners: the margins below follow, and apply to every selected page.`
          : "Drag the box or its corners: the numbers below follow, for exact values."}
      </figcaption>
    </figure>
  );
}
