// Runs the real CLI as a subprocess: from source by default, or the compiled
// binary when PDFVAULT_BIN points at one (CI does both).
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { cp, mkdir, mkdtemp, realpath, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";

const root = new URL("../../../", import.meta.url).pathname;
const fixtures = `${root}fixtures/generated`;
const bin = process.env.PDFVAULT_BIN ? [resolve(process.env.PDFVAULT_BIN)] : ["bun", `${root}packages/cli/src/main.ts`];
let dir: string;

beforeAll(async () => {
  dir = await realpath(await mkdtemp(join(tmpdir(), "pdfvault-cli-")));
  for (const f of ["text-3.pdf", "text-5.pdf", "photo.pdf", "corrupt.pdf", "photo.jpg", "locked.pdf"]) {
    await cp(`${fixtures}/${f}`, `${dir}/${f}`);
  }
});
afterAll(() => rm(dir, { recursive: true, force: true }));

function cli(args: string[], stdin?: Uint8Array, env?: Record<string, string>) {
  const p = Bun.spawnSync([...bin, ...args], { cwd: dir, stdin: stdin ?? "ignore", env: env ? { ...process.env, ...env } : undefined });
  return { code: p.exitCode, stdout: p.stdout, out: p.stdout.toString(), err: p.stderr.toString() };
}

describe("usage", () => {
  test("help and version", () => {
    expect(cli([]).code).toBe(2);
    const h = cli(["--help"]);
    expect(h.code).toBe(0);
    expect(h.out).toContain("compress");
    expect(cli(["compress", "--help"]).out).toContain("--preset <lossless|balanced|strong|extreme>");
    expect(cli(["--version"]).out.trim()).toMatch(/^\d+\.\d+\.\d+/);
    expect(cli(["compress", "--help"]).out).toContain("--grayscale, --gray");
  });

  test("usage errors exit 2", () => {
    expect(cli(["nope"]).code).toBe(2);
    expect(cli(["compress"]).code).toBe(2);
    expect(cli(["compress", "missing.pdf"]).code).toBe(2);
    expect(cli(["compress", "text-3.pdf", "--preset", "max"]).code).toBe(2);
    expect(cli(["compress", "text-3.pdf", "--dpi", "abc"]).code).toBe(2);
    expect(cli(["compress", "text-3.pdf", "--bogus"]).code).toBe(2);
    expect(cli(["compress", "photo.jpg"]).err).toContain("is not a PDF");
    expect(cli(["watermark", "text-3.pdf"]).err).toContain("--text is required");
  });

  test("list is machine readable", () => {
    const tools = JSON.parse(cli(["list"]).out) as { id: string }[];
    expect(tools.map((t) => t.id)).toContain("to-epub");
  });
});

describe("running", () => {
  test("compress writes next to the input and reports in --json", () => {
    const r = cli(["compress", "photo.pdf", "--json"]);
    expect(r.code).toBe(0);
    const body = JSON.parse(r.out);
    expect(body).toMatchObject({ command: "compress", ok: true });
    const res = body.results[0];
    expect(res.outputs[0].path).toBe(join(dir, "photo-compressed.pdf"));
    expect(res.report.outputBytes).toBeLessThan(res.report.inputBytes);
  });

  test("never overwrites the input, and existing outputs only with --force", () => {
    expect(cli(["compress", "text-3.pdf", "-o", "text-3.pdf"]).err).toContain("refusing to overwrite the input");
    expect(cli(["rotate", "text-3.pdf", "-o", "r.pdf"]).code).toBe(0);
    const again = cli(["rotate", "text-3.pdf", "-o", "r.pdf"]);
    expect(again.code).toBe(1);
    expect(again.err).toContain("--force");
    expect(cli(["rotate", "text-3.pdf", "-o", "r.pdf", "--force"]).code).toBe(0);
  });

  test("short aliases map to their option", () => {
    const r = cli(["compress", "photo.pdf", "--gray", "-o", "gray.pdf", "--json"]);
    expect(r.code).toBe(0);
  });

  test("a failing file exits 1 but the others still run", () => {
    const r = cli(["compress", "corrupt.pdf", "text-5.pdf", "--json", "-o", "batch/"]);
    expect(r.code).toBe(1);
    const body = JSON.parse(r.out);
    expect(body.ok).toBe(false);
    expect(body.results.map((x: { ok: boolean }) => x.ok)).toEqual([false, true]);
    expect(body.results[0].error.code).toBe("invalid_pdf");
  });

  test("globs batch every match", () => {
    const r = cli(["rotate", "text-*.pdf", "-o", "rotated/", "--json"]);
    expect(r.code).toBe(0);
    expect(JSON.parse(r.out).results).toHaveLength(2);
  });

  test("stdin to stdout", () => {
    const input = new Uint8Array(Bun.file(`${fixtures}/text-3.pdf`).size);
    const bytes = Bun.spawnSync(["cat", `${fixtures}/text-3.pdf`]).stdout;
    input.set(bytes);
    const r = cli(["extract", "-", "--pages", "2", "-o", "-"], input);
    expect(r.code).toBe(0);
    expect(r.stdout.subarray(0, 5).toString()).toBe("%PDF-");
    const info = cli(["info", "-", "--json"], new Uint8Array(r.stdout));
    expect(JSON.parse(info.out).results[0].report.pages).toBe(1);
  });

  test("two-word aliases and report-only commands", () => {
    expect(cli(["pages", "delete", "text-5.pdf", "--pages", "1", "-o", "d.pdf"]).code).toBe(0);
    expect(cli(["img", "compress", "photo.jpg", "--quality", "40", "-o", "small.jpg"]).code).toBe(0);
    const meta = cli(["meta", "text-3.pdf"]);
    expect(meta.code).toBe(0);
    expect(meta.out).toContain('"Title": "Fixture"');
  });

  test("passwords", () => {
    const locked = cli(["info", "locked.pdf", "--json"]);
    expect(JSON.parse(locked.out).results[0].report.security.needsPassword).toBe(true);
    expect(cli(["decrypt", "locked.pdf", "--password", "wrong"]).code).toBe(1);
    expect(cli(["decrypt", "locked.pdf", "--password", "secret", "-o", "open.pdf"]).code).toBe(0);
  });

  test("split writes one file per part into a directory", () => {
    const r = cli(["split", "text-5.pdf", "--every", "2", "-o", "parts/", "--json"]);
    const outs = JSON.parse(r.out).results[0].outputs.map((o: { path: string }) => o.path.split("/").pop());
    expect(outs).toEqual(["text-5-part-1-2.pdf", "text-5-part-3-4.pdf", "text-5-part-5-5.pdf"]);
  });
});

describe("agent skill", () => {
  test("prints the skill that ships with this version", async () => {
    const r = cli(["skill"]);
    expect(r.code).toBe(0);
    const repo = await Bun.file(`${root}skills/pdfvault/SKILL.md`).text();
    expect(r.out.trimEnd()).toBe(repo.trimEnd());
    expect(cli(["--help"]).out).toContain("skill");
  });

  test("installs next to npx skills, links agents present, and is idempotent", async () => {
    const home = await realpath(await mkdtemp(join(tmpdir(), "pdfvault-home-")));
    await mkdir(`${home}/.claude/skills`, { recursive: true });
    await mkdir(`${home}/.codex/skills`, { recursive: true });
    try {
      const first = JSON.parse(cli(["skill", "--install", "--json"], undefined, { HOME: home }).out);
      expect(first.placements.map((p: { action: string }) => p.action)).toEqual(["installed", "linked", "linked"]);
      const md = await Bun.file(`${home}/.claude/skills/pdfvault/SKILL.md`).text();
      expect(md).toStartWith("---\nname: pdfvault");
      expect(md).toContain(`installed by pdfvault ${first.version}`);
      expect(await Bun.file(`${home}/.codex/skills/pdfvault/SKILL.md`).text()).toBe(md);
      const again = JSON.parse(cli(["skill", "--install", "--json"], undefined, { HOME: home }).out);
      expect(again.placements.every((p: { action: string }) => p.action === "unchanged")).toBe(true);
      // A stale copy from an older version is refreshed.
      await Bun.write(`${home}/.agents/skills/pdfvault/SKILL.md`, "old");
      const update = JSON.parse(cli(["skill", "--install", "--json"], undefined, { HOME: home }).out);
      expect(update.placements[0].action).toBe("updated");
    } finally {
      await rm(home, { recursive: true, force: true });
    }
  });

  test("--dir writes a plain copy; bad flags are usage errors", async () => {
    const r = JSON.parse(cli(["skill", "--install", "--dir", "agent-skills", "--json"]).out);
    expect(r.placements[0]).toMatchObject({ path: join(dir, "agent-skills/pdfvault"), action: "installed" });
    expect(cli(["skill", "--dir", "x"]).code).toBe(2);
    expect(cli(["skill", "--nope"]).code).toBe(2);
  });
});
