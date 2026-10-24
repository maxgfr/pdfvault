// Bun loader for tests: the freshly built wasm and the fetched models.
import { Engine, type Loader } from "../src/index.ts";

const root = new URL("../../../", import.meta.url).pathname;

export const bunLoader: Loader = {
  wasm: async (name) => Bun.file(`${root}packages/engine/wasm/${name}/${name}_bg.wasm`).arrayBuffer(),
  model: async (name) => new Uint8Array(await Bun.file(`${root}models/${name}`).arrayBuffer()),
  font: async (name) => new Uint8Array(await Bun.file(`${root}models/fonts/${name}`).arrayBuffer()),
};

export const engine = new Engine(bunLoader);

export async function fixture(name: string): Promise<Uint8Array> {
  const file = Bun.file(`${root}fixtures/generated/${name}`);
  if (!(await file.exists())) throw new Error(`fixtures/generated/${name} missing: run \`bun run fixtures\``);
  return new Uint8Array(await file.arrayBuffer());
}
