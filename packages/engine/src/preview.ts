// The editor's exact preview: the edits that change what is already on a page
// (rewritten paragraphs, moved or deleted objects) applied by the engine, and
// the pages they touch rendered, so what the editor shows is what it saves.

import type { Engine, OutFile } from "./engine.ts";
import type { EditOps, EditReport } from "./edit.ts";
import { toolById, withDefaults } from "./tools.ts";

export interface PreviewPage {
  /** 0-based source page. */
  page: number;
  file: OutFile;
}

export async function previewPages(
  engine: Engine,
  inputs: Uint8Array[],
  ops: EditOps,
  pages: number[],
  dpi: number,
  password?: string,
): Promise<{ pages: PreviewPage[]; report: EditReport }> {
  const tool = toolById.get("edit")!;
  const { order: _order, ...edits } = ops;
  const edited = await tool.run(engine, inputs, withDefaults(tool, { ops: JSON.stringify(edits), password }));
  const bytes = edited.files[0]!.bytes;
  const out = await engine.render<unknown>("render", [bytes], { format: "jpeg", dpi, quality: 82, pages: pages.map((p) => p + 1).join(",") });
  return { pages: out.files.map((file, i) => ({ page: pages[i]!, file })), report: edited.report as EditReport };
}
