#!/usr/bin/env bun
// pdfvault CLI: every command comes from the engine's tool registry, so it
// runs exactly what the site runs, on the same wasm.

import { basename, dirname, join, resolve } from "node:path";
import { stat } from "node:fs/promises";
import {
  Engine,
  EngineError,
  outputName,
  toolById,
  tools,
  withDefaults,
  type Field,
  type Options,
  type OutFile,
  type Tool,
} from "@pdfvault/engine";
import { loader } from "./loader.ts";
import { SKILL, install } from "./skill.ts";
import pkg from "../package.json" with { type: "json" };

const EXIT_OK = 0;
const EXIT_FAILED = 1;
const EXIT_USAGE = 2;

class UsageError extends Error {}

interface Input {
  name: string;
  path: string | null;
  bytes: Uint8Array;
}

interface JobResult {
  input: string;
  ok: boolean;
  outputs?: { path: string; bytes: number }[];
  report?: unknown;
  error?: { code: string; message: string };
}

const GLOBAL_FLAGS = new Set(["output", "json", "force", "quiet", "help", "version"]);

const kebab = (s: string) => s.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
const camel = (s: string) => s.replace(/-([a-z])/g, (_, c: string) => c.toUpperCase());

function resolveCommand(argv: string[]): { tool: Tool; rest: string[] } | null {
  const [a, b] = argv;
  if (!a) return null;
  if (b) {
    const two = tools.find((t) => t.aliases?.includes(`${a} ${b}`));
    if (two) return { tool: two, rest: argv.slice(2) };
  }
  const one = toolById.get(a) ?? tools.find((t) => t.aliases?.includes(a));
  return one ? { tool: one, rest: argv.slice(1) } : null;
}

interface Parsed {
  positional: string[];
  options: Options;
  output?: string;
  json: boolean;
  force: boolean;
  quiet: boolean;
  help: boolean;
}

function parseArgs(tool: Tool, args: string[]): Parsed {
  const fields = new Map<string, Field>(tool.fields.map((f) => [f.name, f]));
  for (const f of tool.fields) if (f.cliAlias) fields.set(camel(f.cliAlias), f);
  const p: Parsed = { positional: [], options: {}, json: false, force: false, quiet: false, help: false };
  for (let i = 0; i < args.length; i++) {
    const arg = args[i]!;
    if (arg === "--") {
      p.positional.push(...args.slice(i + 1));
      break;
    }
    if (arg === "-" || !arg.startsWith("-")) {
      p.positional.push(arg);
      continue;
    }
    const short: Record<string, string> = { "-o": "output", "-f": "force", "-q": "quiet", "-h": "help" };
    let [flag, value] = arg.startsWith("--") ? (arg.slice(2).split(/=(.*)/s) as [string, string | undefined]) : [short[arg] ?? "", undefined];
    if (!flag) throw new UsageError(`unknown option ${arg}`);
    const negated = flag.startsWith("no-") && !fields.has(camel(flag)) && fields.has(camel(flag.slice(3)));
    if (negated) flag = flag.slice(3);
    let name = camel(flag);
    if (GLOBAL_FLAGS.has(name)) {
      if (name === "output") {
        p.output = value ?? args[++i];
        if (p.output === undefined) throw new UsageError("--output needs a path");
      } else (p as unknown as Record<string, boolean>)[name] = true;
      continue;
    }
    const field = fields.get(name);
    if (!field) throw new UsageError(`unknown option --${flag} for ${tool.id} (see pdfvault ${tool.id} --help)`);
    name = field.name;
    if (field.kind === "boolean") {
      p.options[name] = negated ? false : value === undefined ? true : value !== "false";
      continue;
    }
    if (negated) throw new UsageError(`--no-${flag} only applies to on/off options`);
    value ??= args[++i];
    if (value === undefined) throw new UsageError(`--${flag} needs a value`);
    p.options[name] = coerce(field, value);
  }
  return p;
}

function coerce(field: Field, value: string): string | number {
  if (field.kind === "number") {
    const n = Number(value);
    if (!Number.isFinite(n)) throw new UsageError(`--${kebab(field.name)} expects a number, got "${value}"`);
    if (n < field.min || n > field.max) throw new UsageError(`--${kebab(field.name)} must be between ${field.min} and ${field.max}`);
    return n;
  }
  if (field.kind === "select" && !field.choices.some((c) => c.value === value)) {
    throw new UsageError(`--${kebab(field.name)} must be one of: ${field.choices.map((c) => c.value).join(", ")}`);
  }
  return value;
}

async function expandInputs(patterns: string[]): Promise<string[]> {
  const out: string[] = [];
  for (const pattern of patterns) {
    if (pattern === "-" || !/[*?[{]/.test(pattern)) {
      out.push(pattern);
      continue;
    }
    const matches = [...new Bun.Glob(pattern).scanSync({ cwd: ".", onlyFiles: true })].sort();
    if (matches.length === 0) throw new UsageError(`no file matches ${pattern}`);
    out.push(...matches);
  }
  return out;
}

async function readInput(path: string): Promise<Input> {
  if (path === "-") {
    const bytes = new Uint8Array(await Bun.stdin.arrayBuffer());
    if (bytes.length === 0) throw new UsageError("stdin is empty");
    return { name: "stdin", path: null, bytes };
  }
  const file = Bun.file(path);
  if (!(await file.exists())) throw new UsageError(`${path}: no such file`);
  return { name: basename(path), path: resolve(path), bytes: new Uint8Array(await file.arrayBuffer()) };
}

const isPdf = (b: Uint8Array) => new TextDecoder().decode(b.subarray(0, 1024)).includes("%PDF-");

function checkKinds(tool: Tool, inputs: Input[]) {
  const want = (i: number) => {
    switch (tool.input) {
      case "pdf":
      case "pdfs":
      case "pdf+pdf":
        return "pdf";
      case "image":
      case "images":
        return "image";
      case "pdf+image":
        return i === 0 ? "pdf" : "image";
    }
  };
  inputs.forEach((input, i) => {
    const kind = want(i);
    if (kind === "pdf" && !isPdf(input.bytes)) throw new UsageError(`${input.name} is not a PDF`);
    if (kind === "image" && isPdf(input.bytes)) throw new UsageError(`${input.name} is a PDF; ${tool.id} expects images`);
  });
  const n = inputs.length;
  if (n === 0) throw new UsageError(`${tool.id} needs input files`);
  if (tool.input === "pdfs" && n < 2) throw new UsageError("merge needs at least two PDFs");
  if ((tool.input === "pdf+image" || tool.input === "pdf+pdf") && n !== 2) {
    throw new UsageError(tool.input === "pdf+image" ? `${tool.id} takes a PDF then an image` : `${tool.id} takes exactly two PDFs`);
  }
}

/** Group inputs into jobs: batch tools run once per file. */
function jobs(tool: Tool, inputs: Input[]): Input[][] {
  return tool.input === "pdf" || tool.input === "image" ? inputs.map((i) => [i]) : [inputs];
}

async function isDir(path: string): Promise<boolean> {
  try {
    return (await stat(path)).isDirectory();
  } catch {
    return false;
  }
}

async function destination(p: Parsed, job: Input[], file: OutFile, single: boolean): Promise<string> {
  const first = job[0]!;
  const name = outputName(first.name === "stdin" ? `stdin.${file.ext}` : first.name, file);
  if (p.output && single && !p.output.endsWith("/") && !(await isDir(p.output))) return resolve(p.output);
  const dir = p.output ?? (first.path ? dirname(first.path) : process.cwd());
  return resolve(join(dir, name));
}

const human = (n: number) => (n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(1)} KB` : `${(n / 1048576).toFixed(2)} MB`);

function summary(tool: Tool, job: Input[], report: unknown, outputs: { path: string; bytes: number }[]): string {
  const r = report as Record<string, unknown> | null;
  const src = job.map((i) => i.name).join(" + ");
  if (tool.id === "compress" && r) {
    const before = Number(r.inputBytes);
    const after = Number(r.outputBytes);
    const pct = before ? Math.round((1 - after / before) * 100) : 0;
    const note = r.keptOriginal ? " (already optimal: original kept)" : "";
    return `${src} → ${outputs[0] ? basename(outputs[0].path) : "-"}  ${human(before)} → ${human(after)} (−${pct}%)${note}`;
  }
  if (outputs.length === 0) return `${src}\n${JSON.stringify(report, null, 2)}`;
  return `${src} → ${outputs.map((o) => `${basename(o.path)} (${human(o.bytes)})`).join(", ")}`;
}

function help(tool?: Tool): string {
  if (!tool) {
    const width = Math.max(...tools.map((t) => t.id.length)) + 2;
    const lines = tools.map((t) => `  ${t.id.padEnd(width)}${t.summary}`);
    return `pdfvault ${pkg.version}: private PDF and image tools. Everything runs on this machine.

Usage: pdfvault <command> <files...> [options]

Commands:
${lines.join("\n")}

Agents:
  ${"skill".padEnd(width)}Print the agent skill for this version; --install sets it up for Claude Code, Codex and others

Global options:
  -o, --output <path>  output file, or directory for several outputs; "-" for stdout
  --json               print a machine-readable report on stdout
  -f, --force          overwrite existing output files (never the input)
  -q, --quiet          no progress or summary on stderr
  -h, --help           help for a command

Inputs may be globs ("scans/*.pdf") or "-" for stdin.
Exit codes: 0 success, 1 a file failed, 2 usage error.`;
  }
  const opts = tool.fields.map((f) => {
    const alias = f.cliAlias ? `, --${f.cliAlias}` : "";
    const flag = f.kind === "boolean" ? `--${kebab(f.name)}${alias}` : `--${kebab(f.name)}${alias} <${f.kind === "select" ? f.choices.map((c) => c.value).join("|") : f.kind}>`;
    const def = "default" in f && f.default !== undefined && f.default !== "" ? ` (default: ${f.default})` : "";
    const extra = f.help ? `. ${f.help}` : "";
    return `  ${flag}\n      ${f.label}${def}${extra}`;
  });
  const inputs: Record<string, string> = {
    pdf: "<pdf...>",
    pdfs: "<pdf> <pdf> [pdf...]",
    image: "<image...>",
    images: "<image> [image...]",
    "pdf+image": "<pdf> <image>",
    "pdf+pdf": "<pdf> <pdf>",
  };
  return `pdfvault ${tool.id} ${inputs[tool.input]} [options]

${tool.summary}
${tool.aliases?.length ? `Also: ${tool.aliases.map((a) => `pdfvault ${a}`).join(", ")}\n` : ""}
Options:
${opts.join("\n") || "  (none)"}
  -o, --output <path>, --json, --force, --quiet`;
}

const SKILL_HELP = `pdfvault skill [--install [--dir <path>]] [--json]

Print the agent skill (SKILL.md) that matches this pdfvault version.

  --install     write it to ~/.agents/skills/pdfvault and link it into every
                agent skills directory present (~/.claude/skills, ~/.codex/skills,
                ~/.cursor/skills, ~/.gemini/skills); rerun after upgrading
  --dir <path>  with --install: a plain copy in <path>/pdfvault instead
  --json        report what was written`;

async function skill(args: string[]): Promise<number> {
  let installIt = false;
  let json = false;
  let dir: string | undefined;
  for (let i = 0; i < args.length; i++) {
    const a = args[i]!;
    if (a === "--install") installIt = true;
    else if (a === "--json") json = true;
    else if (a === "--dir" || a.startsWith("--dir=")) {
      dir = a.includes("=") ? a.slice(6) : args[++i];
      if (!dir) {
        process.stderr.write("pdfvault skill: --dir needs a path\n");
        return EXIT_USAGE;
      }
    } else if (a === "--help" || a === "-h") {
      process.stdout.write(SKILL_HELP + "\n");
      return EXIT_OK;
    } else {
      process.stderr.write(`pdfvault skill: unknown option ${a}\n`);
      return EXIT_USAGE;
    }
  }
  if (dir && !installIt) {
    process.stderr.write("pdfvault skill: --dir only applies with --install\n");
    return EXIT_USAGE;
  }
  if (!installIt) {
    process.stdout.write(SKILL.trimEnd() + "\n");
    return EXIT_OK;
  }
  try {
    const placements = await install(pkg.version, { dir });
    if (json) process.stdout.write(JSON.stringify({ command: "skill", ok: true, version: pkg.version, placements }, null, 2) + "\n");
    else for (const p of placements) process.stderr.write(`${p.action.padEnd(9)} ${p.path}\n`);
    return EXIT_OK;
  } catch (e) {
    process.stderr.write(`pdfvault skill: ${e instanceof Error ? e.message : String(e)}\n`);
    return EXIT_FAILED;
  }
}

export async function main(argv: string[]): Promise<number> {
  const out = (s: string) => process.stdout.write(s + "\n");
  const err = (s: string) => process.stderr.write(s + "\n");
  if (argv.length === 0 || argv[0] === "--help" || argv[0] === "-h" || argv[0] === "help") {
    out(help(argv[1] ? resolveCommand(argv.slice(1))?.tool : undefined));
    return argv.length === 0 ? EXIT_USAGE : EXIT_OK;
  }
  if (argv[0] === "--version" || argv[0] === "-v") {
    out(pkg.version);
    return EXIT_OK;
  }
  if (argv[0] === "skill") return skill(argv.slice(1));
  if (argv[0] === "list") {
    out(JSON.stringify(tools.map((t) => ({ id: t.id, title: t.title, input: t.input, aliases: t.aliases ?? [] })), null, 2));
    return EXIT_OK;
  }
  const cmd = resolveCommand(argv);
  if (!cmd) {
    err(`pdfvault: unknown command "${argv[0]}". Run pdfvault --help for the list.`);
    return EXIT_USAGE;
  }
  const { tool } = cmd;
  let p: Parsed;
  let inputs: Input[];
  try {
    p = parseArgs(tool, cmd.rest);
    if (p.help) {
      out(help(tool));
      return EXIT_OK;
    }
    for (const f of tool.fields) {
      if ("required" in f && f.required && p.options[f.name] === undefined) {
        throw new UsageError(`--${kebab(f.name)} is required (${f.label})`);
      }
    }
    inputs = await Promise.all((await expandInputs(p.positional)).map(readInput));
    checkKinds(tool, inputs);
    if (p.output === "-" && jobs(tool, inputs).length > 1) throw new UsageError("-o - writes one file; give a single input");
  } catch (e) {
    if (e instanceof UsageError) {
      err(`pdfvault ${tool.id}: ${e.message}`);
      return EXIT_USAGE;
    }
    throw e;
  }

  const engine = new Engine(loader);
  const options = withDefaults(tool, p.options);
  const results: JobResult[] = [];
  const log = (s: string) => {
    if (!p.quiet) err(s);
  };
  const toStdout = p.output === "-";
  const progress = process.stderr.isTTY && !p.quiet && !p.json
    ? (done: number, total: number, label: string) => process.stderr.write(`\r${label} (${done + 1}/${total})…\x1b[K`)
    : undefined;

  for (const job of jobs(tool, inputs)) {
    const label = job.map((i) => i.name).join(" + ");
    try {
      const result = await tool.run(engine, job.map((i) => i.bytes), options, progress);
      if (progress) process.stderr.write("\r\x1b[K");
      const outputs: { path: string; bytes: number }[] = [];
      if (toStdout) {
        if (result.files.length !== 1) throw new EngineError("invalid_option", `-o - needs exactly one output; this produced ${result.files.length}`);
        await Bun.write(Bun.stdout, result.files[0]!.bytes);
        outputs.push({ path: "-", bytes: result.files[0]!.bytes.length });
      } else {
        const single = result.files.length === 1;
        for (const file of result.files) {
          const path = await destination(p, job, file, single);
          if (job.some((i) => i.path === path)) {
            throw new EngineError("invalid_option", `refusing to overwrite the input ${basename(path)}; choose another -o`);
          }
          if (!p.force && (await Bun.file(path).exists())) {
            throw new EngineError("invalid_option", `${path} exists; pass --force to overwrite it`);
          }
          await Bun.write(path, file.bytes);
          outputs.push({ path, bytes: file.bytes.length });
        }
      }
      results.push({ input: label, ok: true, outputs, report: result.report });
      if (!p.json) {
        const text = summary(tool, job, result.report, outputs);
        // Report-only commands print to stdout; file-writing ones log to stderr.
        if (outputs.length === 0) out(text);
        else log(text);
      }
    } catch (e) {
      if (progress) process.stderr.write("\r\x1b[K");
      const error = e instanceof EngineError ? { code: e.code, message: e.message } : { code: "internal", message: String(e) };
      results.push({ input: label, ok: false, error });
      if (!p.json) err(`pdfvault ${tool.id}: ${label}: ${error.message}`);
    }
  }

  const ok = results.every((r) => r.ok);
  if (p.json) {
    const body = JSON.stringify({ command: tool.id, ok, results }, null, 2);
    if (toStdout) err(body);
    else out(body);
  }
  return ok ? EXIT_OK : EXIT_FAILED;
}

if (import.meta.main) {
  main(process.argv.slice(2)).then(
    (code) => process.exit(code),
    (e: unknown) => {
      process.stderr.write(`pdfvault: ${e instanceof Error ? e.message : String(e)}\n`);
      process.exit(EXIT_FAILED);
    },
  );
}

