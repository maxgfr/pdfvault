import { describe, expect, test } from "bun:test";
import { checkVerifier, importKey, makeVerifier, open, openJson, seal, sealJson } from "./vault-crypto.ts";

const key = (fill: number) => importKey(new Uint8Array(32).fill(fill));

describe("vault crypto", () => {
  test("round trip with a bound id", async () => {
    const k = await key(1);
    const data = crypto.getRandomValues(new Uint8Array(5000));
    const sealed = await seal(k, "file-1", data);
    expect(await open(k, "file-1", sealed)).toEqual(data);
    const meta = await sealJson(k, "meta-1", { name: "contrat.pdf" });
    expect(await openJson(k, "meta-1", meta)).toEqual({ name: "contrat.pdf" });
  });

  test("the raw key bytes are wiped after import", async () => {
    const raw = new Uint8Array(32).fill(9);
    await importKey(raw);
    expect(raw.every((b) => b === 0)).toBe(true);
  });

  test("wrong key, wrong id or tampered data fail", async () => {
    const k = await key(1);
    const sealed = await seal(k, "file-1", new Uint8Array([1, 2, 3]));
    await expect(open(await key(2), "file-1", sealed)).rejects.toThrow();
    await expect(open(k, "file-2", sealed)).rejects.toThrow();
    const tampered = { iv: sealed.iv, ct: sealed.ct.slice() };
    tampered.ct[0]! ^= 1;
    await expect(open(k, "file-1", tampered)).rejects.toThrow();
  });

  test("verifier tells a right passphrase from a wrong one", async () => {
    const v = await makeVerifier(await key(3));
    expect(await checkVerifier(await key(3), v)).toBe(true);
    expect(await checkVerifier(await key(4), v)).toBe(false);
  });

  test("the key cannot be exported", async () => {
    await expect(crypto.subtle.exportKey("raw", await key(5))).rejects.toThrow();
  });
});
