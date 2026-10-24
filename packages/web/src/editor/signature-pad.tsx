import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { Icon } from "../icons.tsx";
import { useWorkspace } from "../workspace.ts";
import type { StampImage } from "./model.ts";

/** Read an image's pixel size; the bytes stay as they are (JPEGs are embedded untouched). */
async function sized(name: string, bytes: Uint8Array): Promise<StampImage> {
  const blob = new Blob([bytes as BlobPart]);
  const bmp = await createImageBitmap(blob);
  const img = { name, bytes, url: URL.createObjectURL(blob), width: bmp.width, height: bmp.height };
  bmp.close();
  return img;
}

/** The ink's bounding box, so the signature carries no empty margin. */
function trimmed(canvas: HTMLCanvasElement): HTMLCanvasElement | null {
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  const { width, height } = canvas;
  const data = ctx.getImageData(0, 0, width, height).data;
  let [x0, y0, x1, y1] = [width, height, -1, -1];
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (data[(y * width + x) * 4 + 3]! > 0) {
        if (x < x0) x0 = x;
        if (x > x1) x1 = x;
        if (y < y0) y0 = y;
        if (y > y1) y1 = y;
      }
    }
  }
  if (x1 < 0) return null;
  const pad = 6;
  const out = document.createElement("canvas");
  out.width = x1 - x0 + 1 + pad * 2;
  out.height = y1 - y0 + 1 + pad * 2;
  out.getContext("2d")?.drawImage(canvas, x0 - pad, y0 - pad, out.width, out.height, 0, 0, out.width, out.height);
  return out;
}

/** A dialog to draw a signature, upload an image, or reuse one of your files. */
export function ImagePicker({ open, onClose, onPick }: { open: boolean; onClose: () => void; onPick: (img: StampImage) => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const file = useRef<HTMLInputElement>(null);
  const drawing = useRef<{ x: number; y: number } | null>(null);
  const [empty, setEmpty] = useState(true);
  const [color, setColor] = useState("#1a2b6d");
  const [problem, setProblem] = useState<string | null>(null);
  const files = useWorkspace().filter((f) => f.kind === "image");

  useEffect(() => {
    const d = dialog.current;
    if (!d) return;
    if (open && !d.open) d.showModal();
    if (!open && d.open) d.close();
  }, [open]);

  const clear = () => {
    const c = canvas.current;
    c?.getContext("2d")?.clearRect(0, 0, c.width, c.height);
    setEmpty(true);
  };

  const at = (e: ReactPointerEvent<HTMLCanvasElement>) => {
    const c = e.currentTarget;
    const r = c.getBoundingClientRect();
    return { x: ((e.clientX - r.left) / r.width) * c.width, y: ((e.clientY - r.top) / r.height) * c.height };
  };

  const take = async (name: string, bytes: Uint8Array) => {
    try {
      onPick(await sized(name, bytes));
      setProblem(null);
      onClose();
    } catch {
      setProblem(`${name} could not be read as an image.`);
    }
  };

  return (
    <dialog ref={dialog} className="ed-dialog" onClose={onClose} aria-labelledby="ed-sign-title">
      <form method="dialog" className="ed-dialog-head">
        <h2 id="ed-sign-title">Add a signature or an image</h2>
        <button type="submit" className="icon" aria-label="Close">
          <Icon name="x" />
        </button>
      </form>
      <p className="note">Draw with a mouse, a finger or a pen. Then click on a page to place it; drag the corner to resize.</p>
      <canvas
        ref={canvas}
        className="ed-sign-pad"
        width={900}
        height={300}
        aria-label="Signature pad"
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId);
          drawing.current = at(e);
        }}
        onPointerMove={(e) => {
          const from = drawing.current;
          const ctx = e.currentTarget.getContext("2d");
          if (!from || !ctx) return;
          const to = at(e);
          ctx.strokeStyle = color;
          ctx.lineWidth = 2.5 + (e.pressure > 0 && e.pointerType === "pen" ? e.pressure * 3 : 1.5);
          ctx.lineCap = "round";
          ctx.lineJoin = "round";
          ctx.beginPath();
          ctx.moveTo(from.x, from.y);
          ctx.lineTo(to.x, to.y);
          ctx.stroke();
          drawing.current = to;
          setEmpty(false);
        }}
        onPointerUp={() => (drawing.current = null)}
        onPointerCancel={() => (drawing.current = null)}
      />
      <div className="actions">
        <label className="ed-color">
          <span>Ink</span>
          <input type="color" value={color} onChange={(e) => setColor(e.currentTarget.value)} aria-label="Ink color" />
        </label>
        <button type="button" className="secondary small" onClick={clear} disabled={empty}>
          Clear
        </button>
        <button
          type="button"
          className="primary"
          disabled={empty}
          onClick={() => {
            const c = canvas.current && trimmed(canvas.current);
            c?.toBlob(async (blob) => {
              if (!blob) return;
              await take("signature.png", new Uint8Array(await blob.arrayBuffer()));
              clear();
            }, "image/png");
          }}
        >
          Use this signature
        </button>
      </div>
      <div className="ed-dialog-alt">
        <button type="button" className="secondary small" onClick={() => file.current?.click()}>
          <Icon name="upload" /> Upload an image
        </button>
        <input
          ref={file}
          type="file"
          hidden
          accept="image/png,image/jpeg,image/webp,image/gif,image/bmp"
          aria-label="Image to place"
          onChange={async (e) => {
            const f = e.currentTarget.files?.[0];
            e.currentTarget.value = "";
            if (f) await take(f.name, new Uint8Array(await f.arrayBuffer()));
          }}
        />
        {files.map((f) => (
          <button key={f.key} type="button" className="chip" onClick={() => void take(f.name, f.bytes)}>
            <Icon name="image" /> {f.name}
          </button>
        ))}
      </div>
      {problem ? <p className="error-text">{problem}</p> : null}
    </dialog>
  );
}
