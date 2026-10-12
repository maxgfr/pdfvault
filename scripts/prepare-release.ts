// Called by semantic-release with the next version: stamp it into the CLI's
// package.json, build the npm bundle and every binary, write SHA256SUMS.
import { createHash } from "node:crypto";
import { readdir } from "node:fs/promises";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+/.test(version)) throw new Error(`bad version "${version}"`);
const root = new URL("..", import.meta.url).pathname;
const pkgPath = `${root}packages/cli/package.json`;
const pkg = await Bun.file(pkgPath).json();
pkg.version = version;
await Bun.write(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

const build = Bun.spawnSync(["bun", `${root}packages/cli/build.ts`, "--binaries"], { stdout: "inherit", stderr: "inherit" });
if (build.exitCode !== 0) process.exit(build.exitCode ?? 1);

const dir = `${root}packages/cli/dist/bin`;
const lines: string[] = [];
for (const name of (await readdir(dir)).filter((n) => n.startsWith("pdfvault-")).sort()) {
  const hash = createHash("sha256").update(new Uint8Array(await Bun.file(`${dir}/${name}`).arrayBuffer())).digest("hex");
  lines.push(`${hash}  ${name}`);
}
await Bun.write(`${dir}/SHA256SUMS`, lines.join("\n") + "\n");
console.log(lines.join("\n"));
