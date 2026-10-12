// Encrypted workspace: files kept in IndexedDB, sealed with AES-256-GCM
// under a key derived from a passphrase. Off by default; while locked the
// key does not exist anywhere, and names are encrypted along with contents.

import { runKdf } from "./runner.ts";
import {
  DEFAULT_KDF,
  checkVerifier,
  importKey,
  makeVerifier,
  open,
  openJson,
  seal,
  sealJson,
  type KdfParams,
  type Sealed,
} from "./vault-crypto.ts";

const DB = "pdfvault-vault";
const AUTO_LOCK_MS = 5 * 60 * 1000;

interface Header {
  salt: Uint8Array;
  kdf: KdfParams;
  verifier: Sealed;
}

interface StoredFile {
  id: string;
  meta: Sealed;
  data: Sealed;
}

export interface VaultFile {
  id: string;
  name: string;
  type: string;
  size: number;
  added: number;
}

function idb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => {
      req.result.createObjectStore("header");
      req.result.createObjectStore("files", { keyPath: "id" });
    };
    req.onsuccess = () => {
      // Let a wipe from another tab proceed.
      req.result.onversionchange = () => req.result.close();
      resolve(req.result);
    };
    req.onerror = () => reject(req.error);
  });
}

async function tx<T>(store: string, mode: IDBTransactionMode, fn: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const db = await idb();
  try {
    return await new Promise<T>((resolve, reject) => {
      const t = db.transaction(store, mode);
      const req = fn(t.objectStore(store));
      t.oncomplete = () => resolve(req.result);
      t.onerror = () => reject(t.error);
      t.onabort = () => reject(t.error);
    });
  } finally {
    db.close();
  }
}

type Listener = () => void;

class Vault {
  #key: CryptoKey | null = null;
  #timer: ReturnType<typeof setTimeout> | undefined;
  #listeners = new Set<Listener>();

  constructor() {
    if (typeof document !== "undefined") {
      document.addEventListener("visibilitychange", () => {
        if (document.visibilityState === "hidden") this.lock();
      });
      for (const ev of ["pointerdown", "keydown"]) addEventListener(ev, () => this.#touch(), { passive: true });
    }
  }

  subscribe(fn: Listener): () => void {
    this.#listeners.add(fn);
    return () => this.#listeners.delete(fn);
  }

  #emit() {
    for (const fn of this.#listeners) fn();
  }

  #touch() {
    if (!this.#key) return;
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => this.lock(), AUTO_LOCK_MS);
  }

  get unlocked(): boolean {
    return this.#key !== null;
  }

  async exists(): Promise<boolean> {
    return (await tx<Header | undefined>("header", "readonly", (s) => s.get("header"))) !== undefined;
  }

  async create(passphrase: string): Promise<void> {
    if (passphrase.length < 8) throw new Error("use at least 8 characters");
    if (await this.exists()) throw new Error("a vault already exists here");
    const salt = crypto.getRandomValues(new Uint8Array(16));
    const key = await importKey(await runKdf(passphrase, salt, DEFAULT_KDF));
    const header: Header = { salt, kdf: DEFAULT_KDF, verifier: await makeVerifier(key) };
    await tx("header", "readwrite", (s) => s.put(header, "header"));
    this.#key = key;
    this.#touch();
    this.#emit();
  }

  async unlock(passphrase: string): Promise<void> {
    const header = await tx<Header | undefined>("header", "readonly", (s) => s.get("header"));
    if (!header) throw new Error("there is no vault on this device");
    const key = await importKey(await runKdf(passphrase, header.salt, header.kdf));
    if (!(await checkVerifier(key, header.verifier))) throw new Error("wrong passphrase");
    this.#key = key;
    this.#touch();
    this.#emit();
  }

  lock() {
    if (!this.#key) return;
    this.#key = null;
    clearTimeout(this.#timer);
    this.#emit();
  }

  #need(): CryptoKey {
    if (!this.#key) throw new Error("the vault is locked");
    this.#touch();
    return this.#key;
  }

  async add(name: string, type: string, bytes: Uint8Array): Promise<VaultFile> {
    const key = this.#need();
    const id = crypto.randomUUID();
    const file: VaultFile = { id, name, type, size: bytes.length, added: Date.now() };
    const stored: StoredFile = { id, meta: await sealJson(key, `meta:${id}`, file), data: await seal(key, `data:${id}`, bytes) };
    await tx("files", "readwrite", (s) => s.put(stored));
    this.#emit();
    return file;
  }

  async list(): Promise<VaultFile[]> {
    const key = this.#need();
    const all = await tx<StoredFile[]>("files", "readonly", (s) => s.getAll());
    const files = await Promise.all(all.map((f) => openJson<VaultFile>(key, `meta:${f.id}`, f.meta)));
    return files.sort((a, b) => b.added - a.added);
  }

  async read(id: string): Promise<Uint8Array> {
    const key = this.#need();
    const f = await tx<StoredFile | undefined>("files", "readonly", (s) => s.get(id));
    if (!f) throw new Error("file not found");
    return open(key, `data:${id}`, f.data);
  }

  async remove(id: string): Promise<void> {
    this.#need();
    await tx("files", "readwrite", (s) => s.delete(id));
    this.#emit();
  }

  /** Emergency wipe: forget the key and delete the whole database. */
  async wipe(): Promise<void> {
    this.#key = null;
    clearTimeout(this.#timer);
    await new Promise<void>((resolve, reject) => {
      const req = indexedDB.deleteDatabase(DB);
      let stuck: ReturnType<typeof setTimeout> | undefined;
      req.onsuccess = () => {
        clearTimeout(stuck);
        resolve();
      };
      req.onerror = () => reject(req.error);
      // Another tab holding the database open delays the delete until it closes.
      req.onblocked = () => {
        stuck = setTimeout(() => reject(new Error("close other pdfvault tabs to finish the wipe")), 5000);
      };
    });
    this.#emit();
  }
}

export const vault = new Vault();
