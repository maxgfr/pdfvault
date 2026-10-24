import { useState } from "react";
import { Icon } from "../icons.tsx";
import { newId, type Doc, type PageRef, type SrcPage } from "./model.ts";
import { setVars } from "./page-canvas.tsx";

interface Props {
  pages: PageRef[];
  srcs: SrcPage[];
  /** How many pages the input has; blank pages come after. */
  inputPages: number;
  current: number;
  onGo: (index: number) => void;
  commit: (fn: (d: Doc) => Doc) => void;
}

const moved = (pages: PageRef[], from: number, to: number) => {
  const next = [...pages];
  const [p] = next.splice(from, 1);
  next.splice(to, 0, p!);
  return next;
};

/** The page strip: drag to reorder (or use the buttons), turn, copy, remove, add blank pages. */
export function Thumbs({ pages, srcs, inputPages, current, onGo, commit }: Props) {
  const [dragging, setDragging] = useState<number | null>(null);
  const [target, setTarget] = useState<number | null>(null);
  const setPages = (fn: (p: PageRef[]) => PageRef[]) => commit((d) => ({ ...d, pages: fn(d.pages) }));

  const addBlank = () =>
    commit((d) => {
      const at = Math.min(current, d.pages.length - 1);
      const near = d.pages[at];
      // A blank of a blank takes the size its neighbour was given.
      const like = near ? (near.src < inputPages ? near.src : d.blanks[near.src - inputPages]?.like) : undefined;
      const src = inputPages + d.blanks.length;
      const pagesNext = [...d.pages];
      pagesNext.splice(at + 1, 0, { id: newId(), src, rotate: 0 });
      return { ...d, blanks: [...d.blanks, { size: "same", like }], pages: pagesNext };
    });

  return (
    <nav className="ed-thumbs" aria-label="Page order">
      <ol>
        {pages.map((p, i) => {
          const src = srcs[p.src];
          return (
            <li
              key={p.id}
              draggable
              data-current={i === current}
              data-target={target === i && dragging !== null && dragging !== i ? (dragging < i ? "after" : "before") : undefined}
              onDragStart={(e) => {
                setDragging(i);
                e.dataTransfer.effectAllowed = "move";
                e.dataTransfer.setData("text/plain", String(i));
              }}
              onDragOver={(e) => {
                if (dragging === null) return;
                e.preventDefault();
                setTarget(i);
              }}
              onDrop={(e) => {
                e.preventDefault();
                e.stopPropagation();
                if (dragging !== null && dragging !== i) setPages((all) => moved(all, dragging, i));
                setDragging(null);
                setTarget(null);
              }}
              onDragEnd={() => {
                setDragging(null);
                setTarget(null);
              }}
            >
              <button type="button" className="ed-thumb" onClick={() => onGo(i)} aria-label={`Go to page ${i + 1}`} aria-current={i === current ? "true" : undefined}>
                <span className="ed-thumb-frame" data-turned={p.rotate % 180 !== 0} ref={(el) => setVars(el, { rot: p.rotate, ratio: src ? src.width / src.height : 0.77 })}>
                  {src?.url ? <img src={src.url} alt="" draggable={false} loading="lazy" /> : <span className="ed-blank" />}
                </span>
                <span className="ed-thumb-n">{i + 1}</span>
              </button>
              <span className="ed-thumb-tools">
                <button type="button" className="icon" onClick={() => setPages((all) => all.map((x, j) => (j === i ? { ...x, rotate: (x.rotate + 270) % 360 } : x)))} aria-label={`Turn page ${i + 1} left`} title="Turn left">
                  <Icon name="rotate-ccw" />
                </button>
                <button type="button" className="icon" onClick={() => setPages((all) => all.map((x, j) => (j === i ? { ...x, rotate: (x.rotate + 90) % 360 } : x)))} aria-label={`Turn page ${i + 1} right`} title="Turn right">
                  <Icon name="rotate-cw" />
                </button>
                <button type="button" className="icon" onClick={() => setPages((all) => [...all.slice(0, i + 1), { ...p, id: newId() }, ...all.slice(i + 1)])} aria-label={`Duplicate page ${i + 1}`} title="Duplicate">
                  <Icon name="copy" />
                </button>
                <button type="button" className="icon" disabled={i === 0} onClick={() => setPages((all) => moved(all, i, i - 1))} aria-label={`Move page ${i + 1} up`} title="Move up">
                  <Icon name="up" />
                </button>
                <button type="button" className="icon" disabled={i === pages.length - 1} onClick={() => setPages((all) => moved(all, i, i + 1))} aria-label={`Move page ${i + 1} down`} title="Move down">
                  <Icon name="down" />
                </button>
                <button type="button" className="icon" disabled={pages.length === 1} onClick={() => setPages((all) => all.filter((_, j) => j !== i))} aria-label={`Delete page ${i + 1}`} title="Delete">
                  <Icon name="trash" />
                </button>
              </span>
            </li>
          );
        })}
      </ol>
      <button type="button" className="secondary small ed-add-blank" onClick={addBlank} aria-label={`Blank page after ${Math.min(current, pages.length - 1) + 1}`}>
        <Icon name="plus" /> Blank page
      </button>
    </nav>
  );
}
