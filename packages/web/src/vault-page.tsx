import { useEffect, useState, useSyncExternalStore } from "react";
import { Icon } from "./icons.tsx";
import { bytes } from "./reports.tsx";
import { download } from "./tool-page.tsx";
import { vault, type VaultFile } from "./vault.ts";

export function VaultPage() {
  const unlocked = useSyncExternalStore((fn) => vault.subscribe(fn), () => vault.unlocked);
  const [exists, setExists] = useState<boolean | null>(null);
  const [pass, setPass] = useState("");
  const [pass2, setPass2] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [files, setFiles] = useState<VaultFile[]>([]);
  const [confirmWipe, setConfirmWipe] = useState(false);

  useEffect(() => {
    const check = () => void vault.exists().then(setExists);
    check();
    return vault.subscribe(check);
  }, []);
  useEffect(() => {
    if (!unlocked) return setFiles([]);
    const refresh = () => void vault.list().then(setFiles).catch(() => setFiles([]));
    refresh();
    return vault.subscribe(refresh);
  }, [unlocked]);

  const act = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
      setPass("");
      setPass2("");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const add = async (list: FileList | null) => {
    for (const f of list ?? []) await vault.add(f.name, f.type, new Uint8Array(await f.arrayBuffer()));
  };

  return (
    <article className="tool vault">
      <header className="tool-head">
        <a href="#/" className="back">
          <Icon name="back" /> All tools
        </a>
        <h1>Encrypted vault</h1>
        <p>
          Optional. Keep files in this browser, encrypted with AES-256-GCM under a key derived from your passphrase (Argon2id). It
          locks itself after 5 minutes idle or when you leave the tab. Without the passphrase nobody, including you, can read it.
        </p>
      </header>

      {exists === null ? null : !unlocked ? (
        <form
          className="options narrow"
          onSubmit={(e) => {
            e.preventDefault();
            if (exists) void act(() => vault.unlock(pass));
            else if (pass !== pass2) setError("the passphrases differ");
            else void act(() => vault.create(pass));
          }}
        >
          <div className="field">
            <label htmlFor="vault-pass">{exists ? "Passphrase" : "Choose a passphrase"}</label>
            <input id="vault-pass" type="password" autoComplete={exists ? "current-password" : "new-password"} value={pass} onChange={(e) => setPass(e.currentTarget.value)} minLength={exists ? 1 : 8} required />
            {!exists ? <small>At least 8 characters. It cannot be recovered.</small> : null}
          </div>
          {!exists ? (
            <div className="field">
              <label htmlFor="vault-pass2">Repeat it</label>
              <input id="vault-pass2" type="password" autoComplete="new-password" value={pass2} onChange={(e) => setPass2(e.currentTarget.value)} required />
            </div>
          ) : null}
          {error ? <p className="error-text" role="alert">{error}</p> : null}
          <div className="actions">
            <button type="submit" className="primary" disabled={busy}>
              <Icon name={exists ? "unlock" : "lock"} /> {busy ? "Deriving key…" : exists ? "Unlock" : "Create vault"}
            </button>
          </div>
        </form>
      ) : (
        <section className="options">
          <div className="actions spread">
            <label className="secondary button">
              <Icon name="file" /> Add files
              <input type="file" multiple hidden onChange={(e) => void add(e.currentTarget.files)} />
            </label>
            <button type="button" className="secondary" onClick={() => vault.lock()}>
              <Icon name="lock" /> Lock now
            </button>
          </div>
          {files.length ? (
            <ul className="files out">
              {files.map((f) => (
                <li key={f.id}>
                  <span className="name">{f.name}</span>
                  <span className="size">{bytes(f.size)}</span>
                  <button type="button" className="secondary small" onClick={async () => download(f.name, await vault.read(f.id))}>
                    <Icon name="download" /> Download
                  </button>
                  <button type="button" className="icon" onClick={() => void vault.remove(f.id)} aria-label={`Delete ${f.name}`}>
                    <Icon name="trash" />
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p className="note">Empty. Add files here, or use “Save to vault” on any result.</p>
          )}
        </section>
      )}

      {exists ? (
        <section className="danger">
          <h2>Emergency wipe</h2>
          <p>Deletes the vault and everything in it from this browser, immediately. There is no undo.</p>
          {confirmWipe ? (
            <div className="actions">
              <button type="button" className="danger-button" onClick={() => void act(() => vault.wipe()).then(() => setConfirmWipe(false))}>
                Yes, wipe everything
              </button>
              <button type="button" className="secondary" onClick={() => setConfirmWipe(false)}>
                Keep it
              </button>
            </div>
          ) : (
            <button type="button" className="danger-button" onClick={() => setConfirmWipe(true)}>
              <Icon name="trash" /> Wipe the vault
            </button>
          )}
        </section>
      ) : null}
    </article>
  );
}
