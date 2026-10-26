import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { Icon } from "../icons.tsx";
import { session, type Quad, type ScanPage } from "./session.ts";

const NAMES = ["Top-left", "Top-right", "Bottom-right", "Bottom-left"];
/** Handle radius on screen, in CSS pixels. */
const HANDLE = 14;
/** The loupe magnifies this much, so the corner can be put on the paper's edge. */
const ZOOM = 2.5;
const LOUPE = 112;

/** Drag the four corners of the sheet onto the paper's corners. */
export function Corners({ page, onClose }: { page: ScanPage; onClose: () => void }) {
  const svg = useRef<SVGSVGElement>(null);
  const [quad, setQuad] = useState<Quad | undefined>(page.corners);
  const [dragging, setDragging] = useState<number | null>(null);
  const [box, setBox] = useState({ w: 1, h: 1 });
  const w = page.width ?? 1;
  const h = page.height ?? 1;

  // Detection or "whole photo" replace the corners from outside.
  useEffect(() => setQuad(page.corners), [page.corners]);

  useEffect(() => {
    const el = svg.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      const r = el.getBoundingClientRect();
      setBox({ w: r.width || 1, h: r.height || 1 });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [page.width]);

  if (!quad || !page.width) {
    return (
      <div className="crop-wait" role="status">
        {page.error ? <p className="error-text">{page.error}</p> : <p>Looking for the sheet…</p>}
      </div>
    );
  }

  const k = w / box.w;
  const toImage = (e: PointerEvent): [number, number] => {
    const r = svg.current!.getBoundingClientRect();
    const x = ((e.clientX - r.left) / r.width) * w;
    const y = ((e.clientY - r.top) / r.height) * h;
    return [Math.min(w, Math.max(0, x)), Math.min(h, Math.max(0, y))];
  };
  const moveTo = (i: number, p: [number, number]) => setQuad((q) => q && q.map((c, j) => (j === i ? p : c)));
  const commit = (q: Quad | undefined) => {
    if (q) session.setCorners(page.key, q);
  };
  const nudge = (i: number, e: KeyboardEvent) => {
    const step = (e.shiftKey ? 0.05 : 0.005) * Math.max(w, h);
    const d = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[e.key];
    if (!d) return;
    e.preventDefault();
    const c = quad[i]!;
    const next = quad.map((p, j): [number, number] =>
      j === i ? [Math.min(w, Math.max(0, c[0] + d[0]!)), Math.min(h, Math.max(0, c[1] + d[1]!))] : p,
    );
    setQuad(next);
    commit(next);
  };

  const points = quad.map((p) => p.join(",")).join(" ");
  const held = dragging === null ? null : quad[dragging]!;
  // The loupe sits above the finger, or below it near the top edge.
  const loupe =
    held &&
    (() => {
      const sx = (held[0] / w) * box.w;
      const sy = (held[1] / h) * box.h;
      const top = sy - LOUPE - 36 < 0 ? sy + 36 : sy - LOUPE - 36;
      return {
        left: Math.min(box.w - LOUPE, Math.max(0, sx - LOUPE / 2)),
        top,
        backgroundImage: `url("${page.photo}")`,
        backgroundSize: `${box.w * ZOOM}px ${box.h * ZOOM}px`,
        backgroundPosition: `${LOUPE / 2 - sx * ZOOM}px ${LOUPE / 2 - sy * ZOOM}px`,
      };
    })();

  return (
    <div className="crop">
      <div className="crop-stage">
        <img src={page.photo} alt="The photo to straighten" draggable={false} />
        <svg
          ref={svg}
          viewBox={`0 0 ${w} ${h}`}
          preserveAspectRatio="none"
          onPointerMove={(e) => {
            if (dragging !== null) moveTo(dragging, toImage(e));
          }}
          onPointerUp={() => {
            if (dragging === null) return;
            setDragging(null);
            commit(quad);
          }}
          onPointerCancel={() => setDragging(null)}
        >
          <path className="crop-shade" fillRule="evenodd" d={`M0 0H${w}V${h}H0Z M${quad.map((p) => p.join(" ")).join(" L")}Z`} />
          <polygon className="crop-edge" points={points} strokeWidth={2 * k} />
          {quad.map((p, i) => (
            <circle
              key={NAMES[i]}
              className="crop-handle"
              cx={p[0]}
              cy={p[1]}
              r={HANDLE * k}
              strokeWidth={2.5 * k}
              tabIndex={0}
              role="button"
              aria-label={`${NAMES[i]} corner. Arrow keys move it, Shift for bigger steps.`}
              data-active={dragging === i}
              onPointerDown={(e) => {
                e.preventDefault();
                svg.current?.setPointerCapture(e.pointerId);
                setDragging(i);
              }}
              onKeyDown={(e) => nudge(i, e)}
            />
          ))}
        </svg>
        {loupe ? <span className="crop-loupe" style={loupe} aria-hidden="true" /> : null}
      </div>
      {page.found === false ? <p className="note">No sheet stood out from the background: drag the corners onto the paper.</p> : null}
      <div className="crop-bar">
        <button type="button" className="secondary small" onClick={() => void session.detect(page.key)}>
          <Icon name="scan" /> Detect again
        </button>
        <button type="button" className="secondary small" onClick={() => session.wholePhoto(page.key)}>
          <Icon name="square" /> Whole photo
        </button>
        <button type="button" className="secondary small" onClick={() => session.turn(page.key)}>
          <Icon name="rotate-cw" /> Turn
        </button>
        <button type="button" className="primary" onClick={onClose}>
          Done
        </button>
      </div>
    </div>
  );
}
