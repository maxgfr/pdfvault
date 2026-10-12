// Encrypted workspace crypto. The passphrase goes through Argon2id (Rust,
// in a worker); the 32 derived bytes become a non-extractable AES-256-GCM
// key and are zeroed. Every record has its own random IV and is bound to its
// id as additional data, so records cannot be swapped around.

export const VERIFIER = "pdfvault-vault-v1";

export interface KdfParams {
  memoryKib: number;
  iterations: number;
}

export const DEFAULT_KDF: KdfParams = { memoryKib: 64 * 1024, iterations: 3 };

export interface Sealed {
  iv: Uint8Array;
  ct: Uint8Array;
}

export async function importKey(raw: Uint8Array): Promise<CryptoKey> {
  try {
    return await crypto.subtle.importKey("raw", raw as BufferSource, { name: "AES-GCM" }, false, ["encrypt", "decrypt"]);
  } finally {
    raw.fill(0);
  }
}

export async function seal(key: CryptoKey, aad: string, plain: Uint8Array): Promise<Sealed> {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const ct = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv, additionalData: new TextEncoder().encode(aad) },
    key,
    plain as BufferSource,
  );
  return { iv, ct: new Uint8Array(ct) };
}

/** Throws if the key is wrong or a single bit was altered. */
export async function open(key: CryptoKey, aad: string, sealed: Sealed): Promise<Uint8Array> {
  const pt = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: sealed.iv as BufferSource, additionalData: new TextEncoder().encode(aad) },
    key,
    sealed.ct as BufferSource,
  );
  return new Uint8Array(pt);
}

export const sealJson = (key: CryptoKey, aad: string, value: unknown) =>
  seal(key, aad, new TextEncoder().encode(JSON.stringify(value)));

export async function openJson<T>(key: CryptoKey, aad: string, sealed: Sealed): Promise<T> {
  return JSON.parse(new TextDecoder().decode(await open(key, aad, sealed))) as T;
}

export const makeVerifier = (key: CryptoKey) => seal(key, "verifier", new TextEncoder().encode(VERIFIER));

export async function checkVerifier(key: CryptoKey, sealed: Sealed): Promise<boolean> {
  try {
    return new TextDecoder().decode(await open(key, "verifier", sealed)) === VERIFIER;
  } catch {
    return false;
  }
}
