// The agent skill ships inside the CLI, so the instructions an agent reads
// always match the binary it runs.
//
//   pdfvault skill                     print SKILL.md
//   pdfvault skill --install           ~/.agents/skills/pdfvault, linked into
//                                      every agent skills directory present
//   pdfvault skill --install --dir D   a plain copy in D/pdfvault
//
// Same layout as `npx skills add maxgfr/pdfvault`.
import { lstat, mkdir, readlink, rm, symlink } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import openai from "../../../skills/pdfvault/agents/openai.yaml" with { type: "text" };
import skillMd from "../../../skills/pdfvault/SKILL.md" with { type: "text" };

export const SKILL = skillMd as unknown as string;
const OPENAI = openai as unknown as string;

/** Agent directories that read skills, relative to $HOME. */
const AGENT_DIRS = [".claude/skills", ".codex/skills", ".cursor/skills", ".gemini/skills"];

export interface Placement {
  path: string;
  action: "installed" | "updated" | "unchanged" | "linked" | "copied";
}

const stamp = (version: string) => `\n<!-- installed by pdfvault ${version} -->\n`;

async function exists(path: string): Promise<boolean> {
  try {
    await lstat(path);
    return true;
  } catch {
    return false;
  }
}

/** Write the skill's files into `dir` (the `pdfvault` folder itself). */
async function writeCopy(dir: string, version: string): Promise<Placement["action"]> {
  const md = SKILL.trimEnd() + "\n" + stamp(version);
  const file = join(dir, "SKILL.md");
  const before = (await Bun.file(file).exists()) ? await Bun.file(file).text() : null;
  if (before === md && (await Bun.file(join(dir, "agents/openai.yaml")).exists())) return "unchanged";
  await mkdir(join(dir, "agents"), { recursive: true });
  await Bun.write(file, md);
  await Bun.write(join(dir, "agents/openai.yaml"), OPENAI);
  return before === null ? "installed" : "updated";
}

export async function install(version: string, opts: { dir?: string; home?: string } = {}): Promise<Placement[]> {
  if (opts.dir) {
    const target = resolve(opts.dir, "pdfvault");
    return [{ path: target, action: await writeCopy(target, version) }];
  }
  const home = opts.home ?? homedir();
  const canonical = join(home, ".agents/skills/pdfvault");
  const placements: Placement[] = [{ path: canonical, action: await writeCopy(canonical, version) }];
  for (const rel of AGENT_DIRS) {
    const agentDir = join(home, rel);
    if (!(await exists(agentDir))) continue;
    const target = join(agentDir, "pdfvault");
    const link = relative(dirname(target), canonical);
    if (await exists(target)) {
      const stat = await lstat(target);
      if (stat.isSymbolicLink()) {
        if (resolve(dirname(target), await readlink(target)) === canonical) {
          placements.push({ path: target, action: "unchanged" });
          continue;
        }
        await rm(target);
      } else {
        // A real folder someone made: keep it, refresh its files.
        placements.push({ path: target, action: await writeCopy(target, version) });
        continue;
      }
    }
    try {
      await symlink(link, target, "dir");
      placements.push({ path: target, action: "linked" });
    } catch {
      // No symlinks (Windows without developer mode): a copy works the same.
      await writeCopy(target, version);
      placements.push({ path: target, action: "copied" });
    }
  }
  return placements;
}
