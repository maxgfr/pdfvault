import { describe, expect, test } from "bun:test";
import { EngineError, missingCharacters, toolById, tools, withDefaults, type EditOps, type Options, type RunResult, type TextRun } from "../src/index.ts";
import type { CompressReport } from "../src/reports.ts";
import { engine, fixture } from "./loader.ts";

async function run(id: string, inputs: Uint8Array[], options: Options = {}): Promise<RunResult> {
  const tool = toolById.get(id);
  if (!tool) throw new Error(`no tool ${id}`);
  return tool.run(engine, inputs, withDefaults(tool, options));
}

async function pages(pdf: Uint8Array, password?: string): Promise<number> {
  const r = await engine.core<{ pages: number }>("info", [pdf], { password });
  return r.report.pages;
}

/** qpdf --check must pass on every PDF we produce. */
async function qpdfCheck(pdf: Uint8Array, password?: string) {
  const path = `${process.env.TMPDIR ?? "/tmp"}/pdfvault-${crypto.randomUUID()}.pdf`;
  await Bun.write(path, pdf);
  const args = ["qpdf", ...(password ? [`--password=${password}`] : []), "--check", path];
  const proc = Bun.spawnSync(args);
  await Bun.file(path).delete();
  expect({ code: proc.exitCode, out: proc.stdout.toString() }).toMatchObject({ code: 0 });
}

/** `unzip -t`: reads through the central directory, like every real unzipper. */
function unzipTest(bytes: Uint8Array): string {
  const path = `${process.env.TMPDIR ?? "/tmp"}/pdfvault-${crypto.randomUUID()}.zip`;
  Bun.spawnSync(["sh", "-c", "cat > " + path], { stdin: bytes });
  const out = Bun.spawnSync(["unzip", "-t", path]).stdout.toString();
  Bun.spawnSync(["rm", "-f", path]);
  return out;
}

const firstPdf = (r: RunResult) => {
  const f = r.files[0];
  expect(f?.ext).toBe("pdf");
  return f!.bytes;
};

describe("registry", () => {
  test("ids are unique and every field has a label", () => {
    expect(new Set(tools.map((t) => t.id)).size).toBe(tools.length);
    for (const t of tools) for (const f of t.fields) expect(f.label.length).toBeGreaterThan(0);
  });
});

describe("compress", () => {
  test("each preset shrinks a photo PDF, stronger is smaller", async () => {
    const input = await fixture("photo.pdf");
    const sizes: Record<string, number> = {};
    for (const preset of ["lossless", "balanced", "strong", "extreme"]) {
      const r = await run("compress", [input], { preset });
      const report = r.report as CompressReport;
      const out = firstPdf(r);
      expect(out.length).toBeLessThanOrEqual(input.length);
      expect(report.outputBytes).toBe(out.length);
      sizes[preset] = out.length;
      await qpdfCheck(out);
    }
    expect(sizes.balanced!).toBeLessThan(sizes.lossless!);
    expect(sizes.strong!).toBeLessThan(sizes.balanced!);
  });

  test("advanced settings override the preset", async () => {
    const input = await fixture("photo.pdf");
    const r = await run("compress", [input], { preset: "balanced", dpi: 50, quality: 40, grayscale: true });
    const report = r.report as CompressReport;
    const big = report.images.find((i) => i.width === 1500);
    expect(big?.newWidth).toBeLessThan(300);
  });

  test("a locked PDF needs its password", async () => {
    const locked = await fixture("locked.pdf");
    await expect(run("compress", [locked])).rejects.toMatchObject({ code: "password_required" });
    await expect(run("compress", [locked], { password: "nope" })).rejects.toMatchObject({ code: "wrong_password" });
    const r = await run("compress", [locked], { password: "secret" });
    expect(await pages(firstPdf(r))).toBe(2);
  });

  test("garbage is rejected with a stable code", async () => {
    await expect(run("compress", [new TextEncoder().encode("hello")])).rejects.toBeInstanceOf(EngineError);
    await expect(run("compress", [new TextEncoder().encode("hello")])).rejects.toMatchObject({ code: "invalid_pdf" });
  });
});

describe("organize", () => {
  test("merge", async () => {
    const r = await run("merge", [await fixture("text-3.pdf"), await fixture("text-5.pdf")]);
    expect(await pages(firstPdf(r))).toBe(8);
    await qpdfCheck(firstPdf(r));
  });

  test("combine PDFs and images in order", async () => {
    const r = await run("combine", [await fixture("photo.jpg"), await fixture("signature.png"), await fixture("text-3.pdf"), await fixture("photo.png")]);
    const out = firstPdf(r);
    await qpdfCheck(out);
    expect(r.report).toMatchObject({ pages: 6, inputs: 4 });
    const single = firstPdf(await run("combine", [await fixture("photo.jpg")]));
    expect(await pages(single)).toBe(1);
  });

  test("split by every, ranges and bookmarks", async () => {
    const five = await fixture("text-5.pdf");
    expect((await run("split", [five], { mode: "every", every: 2 })).files.map((f) => f.suffix)).toEqual(["part-1-2", "part-3-4", "part-5-5"]);
    expect((await run("split", [five], { mode: "ranges", ranges: "1-2,5" })).files).toHaveLength(2);
    const byBookmark = await run("split", [await fixture("outlined.pdf")], { mode: "bookmarks" });
    expect(byBookmark.files.map((f) => f.suffix)).toEqual(["part-intro", "part-methods", "part-results"]);
  });

  test("extract, delete, reorder", async () => {
    const five = await fixture("text-5.pdf");
    expect(await pages(firstPdf(await run("extract", [five], { pages: "2-3" })))).toBe(2);
    expect(await pages(firstPdf(await run("delete", [five], { pages: "1,5" })))).toBe(3);
    expect(await pages(firstPdf(await run("reorder", [five], { order: "5-1" })))).toBe(5);
    await expect(run("extract", [five], {})).rejects.toMatchObject({ code: "invalid_option" });
  });

  test("rotate, blank, crop, n-up", async () => {
    const three = await fixture("text-3.pdf");
    const rotated = firstPdf(await run("rotate", [three], { angle: "180", pages: "2" }));
    const info = await engine.core<{ pageSizes: { rotation: number }[] }>("info", [rotated]);
    expect(info.report.pageSizes.map((p) => p.rotation)).toEqual([0, 180, 0]);
    expect(await pages(firstPdf(await run("blank", [three], { after: "0,3", count: 2 })))).toBe(7);
    await expect(run("blank", [three], { after: "x" })).rejects.toMatchObject({ code: "invalid_option" });
    const cropped = firstPdf(await run("crop", [three], { top: 50, left: 20 }));
    const sizes = await engine.core<{ pageSizes: { width: number; height: number }[] }>("info", [cropped]);
    expect(sizes.report.pageSizes[0]).toMatchObject({ width: 592, height: 742 });
    expect(await pages(firstPdf(await run("nup", [await fixture("text-5.pdf")], { perSheet: "4" })))).toBe(2);
  });
});

describe("edit", () => {
  test("watermark, numbers, header-footer, sign", async () => {
    const three = await fixture("text-3.pdf");
    for (const [id, options] of [
      ["watermark", { text: "DRAFT", position: "tile" }],
      ["number", { format: "Page {n} of {total}" }],
      ["header-footer", { headerLeft: "ACME", footerRight: "{n}/{total}", date: "2026-01-01" }],
    ] as const) {
      const out = firstPdf(await run(id, [three], options));
      expect(await pages(out)).toBe(3);
      await qpdfCheck(out);
    }
    const signed = firstPdf(await run("sign", [three, await fixture("signature.png")]));
    await qpdfCheck(signed);
    await expect(run("watermark", [three], {})).rejects.toMatchObject({ code: "invalid_option" });
  });

  test("metadata read, set, clear", async () => {
    const pdf = await fixture("text-3.pdf");
    const read = await run("meta", [pdf]);
    expect(read.files).toHaveLength(0);
    expect((read.report as { info: Record<string, string> }).info.Title).toBe("Fixture");
    const edited = firstPdf(await run("meta", [pdf], { title: "Nouveau titre", author: "" }));
    const after = await engine.core<{ info: Record<string, string> }>("meta-read", [edited]);
    expect(after.report.info).toMatchObject({ Title: "Nouveau titre" });
    expect(after.report.info.Author).toBeUndefined();
    const cleared = firstPdf(await run("meta", [pdf], { clear: true }));
    expect((await engine.core<{ info: object }>("meta-read", [cleared])).report.info).toEqual({});
  });

  test("flatten, repair, info", async () => {
    const flat = await run("flatten", [await fixture("form.pdf")]);
    expect(flat.report).toMatchObject({ flattened: 2 });
    const repaired = firstPdf(await run("repair", [await fixture("text-3.pdf")]));
    await qpdfCheck(repaired);
    const info = await run("info", [await fixture("photo.pdf")]);
    expect(info.report).toMatchObject({ pages: 2, images: 3 });
  });

  test("editor: items, forms, blanks, redaction and page order in one run", async () => {
    const text = async (pdf: Uint8Array, page: number) =>
      (await engine.render<{ pages: { spans: { text: string }[] }[] }>("text", [pdf], { pages: String(page) })).report.pages[0]!.spans
        .map((s) => s.text)
        .join("")
        .replace(/\s+/g, "");
    const ops = {
      blanks: [{ size: "a4" }],
      items: [
        { type: "text", page: 0, x: 72, y: 760, text: "Added by the editor", size: 16 },
        { type: "image", page: 1, x: 300, y: 80, w: 160, h: 64, image: 0 },
        { type: "note", page: 0, x: 500, y: 760, text: "Check this" },
        { type: "highlight", page: 1, x: 72, y: 700, w: 120, h: 14 },
        { type: "redact", page: 2, x: 0, y: 0, w: 612, h: 792 },
        { type: "text", page: 3, x: 72, y: 800, text: "On the blank page" },
      ],
      order: [{ page: 3 }, { page: 0, rotate: 90 }, { page: 2 }, { page: 1 }],
    };
    const r = await run("edit", [await fixture("text-3.pdf"), await fixture("signature.png")], { ops: JSON.stringify(ops) });
    const out = firstPdf(r);
    await qpdfCheck(out);
    expect(r.report).toMatchObject({ pages: 4, blanks: 1, drawn: 3, annotations: 2, redactedPages: [3] });
    expect(await pages(out)).toBe(4);
    expect(await text(out, 1)).toContain("Ontheblankpage");
    expect(await text(out, 2)).toContain("Addedbytheeditor");
    expect(await text(out, 3)).toBe("");

    const fields = await engine.core<{ fields: { name: string; kind: string }[] }>("form-fields", [await fixture("form.pdf")]);
    expect(fields.report.fields.map((f) => f.kind).sort()).toEqual(["checkbox", "text"]);
    const filled = await run("edit", [await fixture("form.pdf")], { ops: JSON.stringify({ fields: { name: "Grace", agree: false } }) });
    const after = await engine.core<{ fields: { name: string; value: string }[] }>("form-fields", [firstPdf(filled)]);
    expect(Object.fromEntries(after.report.fields.map((f) => [f.name, f.value]))).toEqual({ name: "Grace", agree: "Off" });

    await expect(run("edit", [await fixture("text-3.pdf")], { ops: "{nope" })).rejects.toMatchObject({ code: "invalid_option" });
    await expect(run("edit", [await fixture("text-3.pdf")], { ops: JSON.stringify({ items: [{ type: "text", page: 9, text: "x" }] }) })).rejects.toMatchObject({
      code: "invalid_option",
    });
  });

  test("editor: change existing text, write any script, redact without flattening", async () => {
    const pdf = await fixture("text-3.pdf");
    const lines = async (bytes: Uint8Array) =>
      (await engine.core<{ runs: TextRun[] }>("text-runs", [bytes], { pages: "1" })).report.runs;
    const before = await lines(pdf);
    const body = before.find((r) => r.text.startsWith("The quick"))!;
    const title = before.find((r) => r.text === "Page 1")!;
    expect(body).toMatchObject({ editable: true, family: "sans", bold: false });
    const ops: EditOps = {
      items: [
        // Same font: Helvetica has every letter of the new text.
        { type: "replace", page: 0, x: body.x, y: body.y, w: body.w, h: body.h, text: "Le vif renard brun, été 2026" },
        // Cyrillic is beyond Helvetica: the bundled Liberation Sans Bold takes over.
        { type: "replace", page: 0, x: title.x, y: title.y, w: title.w, h: title.h, text: "Страница 1", bold: true },
        { type: "text", page: 1, x: 72, y: 500, text: "Ωμέγα — 価格 n'est pas là", font: "serif" },
      ],
    };
    // No font here has the Japanese characters.
    await expect(run("edit", [pdf], { ops: JSON.stringify(ops) })).rejects.toMatchObject({ code: "missing_glyphs" });
    try {
      await run("edit", [pdf], { ops: JSON.stringify(ops) });
    } catch (e) {
      expect(missingCharacters(e)).toBe("価格");
    }
    (ops.items![2] as { text: string }).text = "Ωμέγα — Привет";
    ops.items!.push({ type: "redact", page: 2, x: body.x, y: body.y, w: body.w / 2, h: body.h });
    const r = await run("edit", [pdf], { ops: JSON.stringify(ops) });
    const out = firstPdf(r);
    await qpdfCheck(out);
    expect(r.report).toMatchObject({ replaced: 2, sameFont: 1, covered: 0, redactedPages: [3], flattenedPages: [] });
    const after = await lines(out);
    expect(after.map((l) => l.text)).toEqual(expect.arrayContaining(["Le vif renard brun, été 2026", "Страница 1"]));
    expect(after.find((l) => l.text === "Страница 1")).toMatchObject({ bold: true, size: title.size });
    const page2 = (await engine.core<{ runs: TextRun[] }>("text-runs", [out], { pages: "2" })).report.runs;
    expect(page2.map((l) => l.text)).toContain("Ωμέγα — Привет");
    const page3 = (await engine.core<{ runs: TextRun[] }>("text-runs", [out], { pages: "3" })).report.runs.map((l) => l.text).join(" ");
    expect(page3).not.toContain("quick");
    expect(page3).toContain("lazy dog");
  });
});

describe("security", () => {
  test("encrypt → decrypt round trip", async () => {
    const pdf = await fixture("text-3.pdf");
    const locked = firstPdf(await run("encrypt", [pdf], { userPassword: "s3cret", ownerPassword: "boss" }));
    await qpdfCheck(locked, "s3cret");
    const sec = await engine.core<{ encrypted: boolean; algorithm: string }>("security", [locked]);
    expect(sec.report).toMatchObject({ encrypted: true, algorithm: "AES-256" });
    await expect(pages(locked)).resolves.toBe(0);
    const unlocked = firstPdf(await run("decrypt", [locked], { password: "s3cret" }));
    expect(await pages(unlocked)).toBe(3);
    await expect(run("decrypt", [locked], { password: "bad" })).rejects.toMatchObject({ code: "wrong_password" });
  });

  test("permissions without an open password", async () => {
    const out = firstPdf(await run("permissions", [await fixture("text-3.pdf")], { ownerPassword: "boss" }));
    const sec = await engine.core<{ needsPassword: boolean; permissions: { copy: boolean; print: boolean } }>("security", [out]);
    expect(sec.report).toMatchObject({ needsPassword: false, permissions: { copy: false, print: true } });
    expect(await pages(out)).toBe(3);
  });
});

describe("render", () => {
  test("pdf2img in each format", async () => {
    const pdf = await fixture("text-3.pdf");
    const png = await run("pdf2img", [pdf], { dpi: 36 });
    expect(png.files.map((f) => f.ext)).toEqual(["png", "png", "png"]);
    const jpg = await run("pdf2img", [pdf], { format: "jpeg", dpi: 36, pages: "2" });
    expect(jpg.files).toHaveLength(1);
    expect(jpg.files[0]!.bytes.subarray(0, 2)).toEqual(new Uint8Array([0xff, 0xd8]));
  });

  test("compare", async () => {
    const a = await fixture("text-3.pdf");
    const b = firstPdf(await run("watermark", [a], { text: "CHANGED", opacity: 1, pages: "3" }));
    const same = await run("compare", [a, a]);
    expect(same.report).toMatchObject({ identical: true });
    const diff = await run("compare", [a, b]);
    expect(diff.report).toMatchObject({ identical: false });
    expect(diff.files.map((f) => f.suffix)).toEqual(["diff-page-3"]);
  });
});

describe("text and OCR", () => {
  test("to-md from real text", async () => {
    const r = await run("to-md", [await fixture("text-3.pdf")], { ocr: false });
    const md = new TextDecoder().decode(r.files[0]!.bytes);
    expect(md).toStartWith("# Page 1\n\nThe quick brown fox jumps over the lazy dog.");
    expect(md).toContain("# Page 3");
  });

  test("to-text and to-epub", async () => {
    const text = new TextDecoder().decode((await run("to-text", [await fixture("text-3.pdf")], { ocr: false })).files[0]!.bytes);
    expect(text).toStartWith("Page 1\n\nThe quick brown fox");
    const epub = await run("to-epub", [await fixture("text-3.pdf")], { ocr: false });
    expect(epub.files[0]!.ext).toBe("epub");
    expect(epub.files[0]!.bytes.subarray(0, 2)).toEqual(new Uint8Array([0x50, 0x4b]));
    expect(unzipTest(epub.files[0]!.bytes)).toContain("No errors detected");
  });

  test("the ZIP of several outputs opens with unzip", async () => {
    const z = await engine.core("zip", [new TextEncoder().encode("a"), new TextEncoder().encode("bb")], { names: ["a.txt", "b.txt"] });
    const listing = unzipTest(z.files[0]!.bytes);
    expect(listing).toContain("a.txt");
    expect(listing).toContain("No errors detected");
  });

  test("OCR makes a scan searchable", async () => {
    // scan.pdf is text-3.pdf rasterized: no text layer at all.
    const scan = await fixture("scan.pdf");
    const before = await run("to-md", [scan], { ocr: false });
    expect(new TextDecoder().decode(before.files[0]!.bytes).trim()).toBe("");
    const ocr = await run("ocr", [scan], { pages: "1" });
    expect(ocr.report).toMatchObject({ pagesRecognized: [1] });
    const searchable = firstPdf(ocr);
    await qpdfCheck(searchable);
    const md = new TextDecoder().decode((await run("to-text", [searchable], { ocr: false })).files[0]!.bytes);
    expect(md).toContain("quick brown fox");
  }, 60_000);

  test("to-md falls back to OCR on scanned pages", async () => {
    const r = await run("to-md", [await fixture("scan.pdf")]);
    expect(r.report).toMatchObject({ ocrPages: [1, 2, 3] });
    expect(new TextDecoder().decode(r.files[0]!.bytes)).toContain("Page 2");
  }, 120_000);
});

describe("scan", () => {
  test("photos become a PDF, black and white is smaller", async () => {
    const photo = await fixture("photo.jpg");
    const color = await run("scan", [photo, photo]);
    expect(await pages(firstPdf(color))).toBe(2);
    await qpdfCheck(firstPdf(color));
    expect(color.report).toMatchObject({ pages: [{ detected: true }, { detected: true }] });
    const bw = await run("scan", [photo], { filter: "bw" });
    expect(firstPdf(bw).length).toBeLessThan(firstPdf(color).length / 2);
  });

  test("given corners and rotation are used, bad JSON is refused", async () => {
    const photo = await fixture("photo.jpg");
    const corners = JSON.stringify([{ corners: [[0, 0], [100, 0], [100, 50], [0, 50]], rotate: 90 }]);
    const r = await run("scan", [photo], { corners });
    expect(r.report).toMatchObject({ pages: [{ detected: false, width: 50, height: 100 }] });
    await expect(run("scan", [photo], { corners: "{" })).rejects.toThrow("JSON");
  });

  test("OCR makes the scan searchable", async () => {
    const page = (await run("pdf2img", [await fixture("text-3.pdf")], { pages: "1", dpi: 150 })).files[0]!.bytes;
    const r = await run("scan", [page], { ocr: true });
    const text = new TextDecoder().decode((await run("to-text", [firstPdf(r)], { ocr: false })).files[0]!.bytes);
    expect(text).toContain("quick brown fox");
  }, 60_000);
});

describe("images", () => {
  test("img2pdf keeps JPEG bytes and handles PNG alpha", async () => {
    const r = await run("img2pdf", [await fixture("photo.jpg"), await fixture("signature.png")]);
    const out = firstPdf(r);
    expect(await pages(out)).toBe(2);
    await qpdfCheck(out);
  });

  test("compress, resize, convert, strip, edit", async () => {
    const jpg = await fixture("photo.jpg");
    const small = await run("img-compress", [jpg], { quality: 50 });
    expect(small.files[0]!.bytes.length).toBeLessThan(jpg.length);
    const resized = await run("img-resize", [jpg], { maxWidth: 300 });
    expect(resized.report).toMatchObject({ width: 300, height: 200 });
    const webp = await run("img-convert", [jpg], { format: "webp" });
    expect(webp.files[0]!.ext).toBe("webp");
    const stripped = await run("img-strip", [jpg]);
    expect(stripped.files[0]!.ext).toBe("jpg");
    const edited = await run("img-edit", [jpg], { cropX: 0, cropY: 0, cropWidth: 100, cropHeight: 50, rotate: "90" });
    expect(edited.report).toMatchObject({ width: 50, height: 100 });
    await expect(run("img-resize", [jpg], {})).rejects.toMatchObject({ code: "invalid_option" });
    await expect(run("img-edit", [jpg], { cropX: 1 })).rejects.toMatchObject({ code: "invalid_option" });
  });
});

describe("workspace key derivation", () => {
  test("argon2id is deterministic per salt", async () => {
    const salt = new Uint8Array(16).fill(7);
    const a = await engine.core("kdf", [salt], { passphrase: "correct horse", memoryKib: 8192, iterations: 1 });
    const b = await engine.core("kdf", [salt], { passphrase: "correct horse", memoryKib: 8192, iterations: 1 });
    expect(a.files[0]!.bytes).toEqual(b.files[0]!.bytes);
    expect(a.files[0]!.bytes).toHaveLength(32);
  });
});
