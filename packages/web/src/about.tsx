import { MODEL_PACKS, megabytes, modelPackIds, packBytes, type ModelPackId } from "@pdfvault/engine";
import { useEffect, useState } from "react";
import { Icon } from "./icons.tsx";
import { isInstalled, removePack } from "./models.ts";

/** The downloaded OCR models, each with a Delete button. */
function StoredModels() {
  const [installed, setInstalled] = useState<Partial<Record<ModelPackId, boolean>>>({});
  const refresh = () =>
    void Promise.all(modelPackIds.map(async (id) => [id, await isInstalled(id).catch(() => false)] as const)).then((rows) =>
      setInstalled(Object.fromEntries(rows)),
    );
  useEffect(refresh, []);
  return (
    <ul className="files">
      {modelPackIds.map((id) => (
        <li key={id}>
          <span className="name">
            {MODEL_PACKS[id].title} <small className="hint">({MODEL_PACKS[id].license})</small>
          </span>
          <span className="size">{installed[id] ? megabytes(packBytes(id)) : "Not downloaded"}</span>
          {installed[id] ? (
            <button type="button" className="secondary small" onClick={() => void removePack(id).then(refresh)}>
              <Icon name="trash" /> Delete
            </button>
          ) : null}
        </li>
      ))}
    </ul>
  );
}

export function About() {
  return (
    <article className="tool prose">
      <header className="tool-head">
        <a href="#/" className="back">
          <Icon name="back" /> All tools
        </a>
        <h1>How pdfvault keeps files private</h1>
      </header>
      <h2>Nothing is uploaded</h2>
      <p>
        Every tool runs on your device: a Rust engine compiled to WebAssembly, in a worker inside this tab. Files are read
        from your disk into memory and written back as downloads. There is no server to send them to. When a job ends its
        worker is terminated, which frees all of its memory.
      </p>
      <h2>The browser enforces it</h2>
      <p>
        A Content Security Policy only lets this page connect to its own address, forbids inline and third-party scripts,
        forms and plugins, and requires Trusted Types. There are no analytics, fonts or scripts from anywhere else. Open
        your browser's network tab while you work: after the page loads, nothing goes out. The one exception is an OCR
        model you choose to download from Hugging Face (GLM-OCR, too large to serve from here): the policy also allows
        huggingface.co and its download servers, and the page fetches the model's files from there only after you agree.
        Those requests carry no file of yours; your documents never leave this device.
      </p>
      <h2>Offline</h2>
      <p>
        A service worker keeps the app on your device after the first visit, so it works in airplane mode. The page renderer
        and the fonts are cached the first time you use a tool that needs them.
      </p>
      <h2>Stored on this device</h2>
      <p>
        Text recognition (OCR) needs a model that is not part of the app. The first time a tool needs it, pdfvault asks
        before downloading it from this site; it is checked against a fixed checksum and kept here, so OCR works offline
        afterwards. GLM-OCR, an optional, much larger model for Markdown with tables and formulas, comes from Hugging Face
        the same way, and runs on your graphics card (WebGPU). Delete them whenever you like: each is downloaded again,
        after asking, the next time.
      </p>
      <StoredModels />
      <h2>Verifiable</h2>
      <p>
        The site is built by GitHub Actions from the public repository, with a signed build provenance attestation you can
        check with <code>gh attestation verify</code>. The same engine powers the <code>pdfvault</code> command-line tool:{" "}
        <code>bunx pdfvault compress file.pdf</code>.
      </p>
      <h2>Limits</h2>
      <ul>
        <li>
          OCR reads 50 languages: Latin scripts with their accents, Chinese and Japanese, written horizontally. Its models are
          PP-OCRv6 (Apache-2.0, PaddlePaddle). GLM-OCR (MIT, Z.ai, run with Transformers.js and ONNX Runtime) only writes
          Markdown, text and EPUB: it gives no word positions for a searchable PDF, and needs WebGPU on a computer.
        </li>
        <li>The “extreme” compression level turns every page into an image: text is no longer selectable.</li>
        <li>Fonts are not subset, so text-heavy PDFs shrink less than image-heavy ones.</li>
        <li>Visual signatures are images, not cryptographic signatures.</li>
        <li>
          The editor writes text with the Liberation fonts (SIL Open Font License 1.1, <a href="fonts/LICENSE.txt">licence</a>) when a
          document's own font lacks the letters. It changes text line by line, without reflowing paragraphs.
        </li>
      </ul>
      <p>
        <a href="https://github.com/maxgfr/pdfvault" rel="noopener noreferrer">
          <Icon name="code" /> Source code on GitHub
        </a>
      </p>
    </article>
  );
}
