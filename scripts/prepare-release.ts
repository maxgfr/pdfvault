// Called by semantic-release with the next version: stamp it into the CLI's
// package.json and build the npm bundle. The binaries are built afterwards,
// per OS, by the release workflow (scripts/cli-binaries.ts).
const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+/.test(version)) throw new Error(`bad version "${version}"`);
const root = new URL("..", import.meta.url).pathname;
const pkgPath = `${root}packages/cli/package.json`;
const pkg = await Bun.file(pkgPath).json();
pkg.version = version;
await Bun.write(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

const build = Bun.spawnSync(["bun", `${root}packages/cli/build.ts`], { stdout: "inherit", stderr: "inherit" });
if (build.exitCode !== 0) process.exit(build.exitCode ?? 1);
