// What can be done with the files you picked. Quick actions cover the common
// jobs; every registry tool whose input fits the selection is offered too, so
// starting from files never hides a tool.

import { categories, toolById, tools, type Category, type Tool } from "@pdfvault/engine";
import type { Group, InFile } from "./action-panel.tsx";
import type { WorkFile } from "./workspace.ts";

export interface WorkAction {
  id: string;
  label: string;
  icon: string;
  /** Tool groups to run, or a route to open (the editor). */
  plan: { groups: Group[] } | { route: string; file: string };
}

const asIn = (f: WorkFile): InFile => ({ key: f.key, name: f.name, bytes: f.bytes, password: f.password });
const tool = (id: string) => toolById.get(id)!;

/** The groups that run `t` on the selection, or null when it does not fit. */
export function fit(t: Tool, sel: WorkFile[]): Group[] | null {
  const pdfs = sel.filter((f) => f.kind === "pdf");
  const images = sel.filter((f) => f.kind === "image");
  const onlyPdfs = sel.length > 0 && pdfs.length === sel.length;
  const onlyImages = sel.length > 0 && images.length === sel.length;
  switch (t.input) {
    case "pdf":
      return onlyPdfs ? [{ tool: t, slots: [pdfs.map(asIn)] }] : null;
    case "pdfs":
      return onlyPdfs && pdfs.length >= 2 ? [{ tool: t, slots: [pdfs.map(asIn)] }] : null;
    case "image":
    case "images":
      return onlyImages ? [{ tool: t, slots: [images.map(asIn)] }] : null;
    case "mixed":
      return pdfs.length + images.length === sel.length && sel.length >= 2 ? [{ tool: t, slots: [sel.map(asIn)] }] : null;
    case "pdf+image":
      return sel.length === 2 && pdfs.length === 1 && images.length === 1 ? [{ tool: t, slots: [[asIn(pdfs[0]!)], [asIn(images[0]!)]] }] : null;
    case "pdf+pdf":
      return onlyPdfs && pdfs.length === 2 ? [{ tool: t, slots: [[asIn(pdfs[0]!)], [asIn(pdfs[1]!)]] }] : null;
    case "pdf+images":
      return pdfs.length === 1 && pdfs.length + images.length === sel.length ? [{ tool: t, slots: [[asIn(pdfs[0]!)], images.map(asIn)] }] : null;
    case "markdown":
      // The workspace holds PDFs and images only; Markdown goes through the tool page.
      return null;
  }
}

/** The handful of buttons shown first, in order. */
export function quickActions(sel: WorkFile[]): WorkAction[] {
  const pdfs = sel.filter((f) => f.kind === "pdf");
  const images = sel.filter((f) => f.kind === "image");
  const usable = pdfs.length + images.length === sel.length && sel.length > 0;
  const out: WorkAction[] = [];
  if (usable) {
    const groups: Group[] = [];
    if (pdfs.length) groups.push({ tool: tool("compress"), slots: [pdfs.map(asIn)], label: `${pdfs.length} PDF${pdfs.length > 1 ? "s" : ""}` });
    if (images.length) groups.push({ tool: tool("img-compress"), slots: [images.map(asIn)], label: `${images.length} image${images.length > 1 ? "s" : ""}` });
    out.push({ id: "compress", label: "Compress", icon: "compress", plan: { groups } });
  }
  if (pdfs.length === 1 && sel.length === 1) {
    out.push({ id: "view", label: "View", icon: "view", plan: { route: "#/view", file: pdfs[0]!.key } });
    out.push({ id: "edit", label: "Edit", icon: "edit", plan: { route: "#/edit", file: pdfs[0]!.key } });
  }
  if (images.length === 1 && sel.length === 1) out.push({ id: "img-edit", label: "Crop & rotate", icon: "edit", plan: { groups: fit(tool("img-edit"), sel)! } });
  if (usable && sel.length >= 2) out.push({ id: "combine", label: "Combine into one PDF", icon: "organize", plan: { groups: fit(tool("combine"), sel)! } });
  if (images.length === 1 && sel.length === 1) out.push({ id: "to-pdf", label: "Make a PDF", icon: "convert", plan: { groups: fit(tool("img2pdf"), sel)! } });
  if (pdfs.length && pdfs.length === sel.length) out.push({ id: "encrypt", label: "Protect", icon: "lock", plan: { groups: fit(tool("encrypt"), sel)! } });
  return out;
}

/** Every registry tool that fits, grouped by category, minus the quick ones. */
export function moreActions(sel: WorkFile[], quick: WorkAction[]): { id: Category; title: string; actions: WorkAction[] }[] {
  const shown = new Set(quick.flatMap((q) => ("groups" in q.plan ? q.plan.groups.map((g) => g.tool.id) : [q.id])));
  return categories
    .map((c) => ({
      ...c,
      actions: tools
        .filter((t) => t.category === c.id && !shown.has(t.id))
        .flatMap((t): WorkAction[] => {
          const groups = fit(t, sel);
          if (!groups) return [];
          if (t.id === "edit") {
            const pdf = sel.find((f) => f.kind === "pdf")!;
            return [{ id: t.id, label: t.title, icon: c.id, plan: { route: "#/edit", file: pdf.key } }];
          }
          return [{ id: t.id, label: t.title, icon: c.id, plan: { groups } }];
        }),
    }))
    .filter((c) => c.actions.length);
}
