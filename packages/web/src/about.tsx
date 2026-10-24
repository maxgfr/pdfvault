import { Icon } from "./icons.tsx";

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
        your browser's network tab while you work: after the page loads, nothing goes out.
      </p>
      <h2>Offline</h2>
      <p>
        A service worker keeps the app on your device after the first visit, so it works in airplane mode. The page renderer
        and the OCR models are cached the first time you use a tool that needs them.
      </p>
      <h2>Verifiable</h2>
      <p>
        The site is built by GitHub Actions from the public repository, with a signed build provenance attestation you can
        check with <code>gh attestation verify</code>. The same engine powers the <code>pdfvault</code> command-line tool:{" "}
        <code>bunx pdfvault compress file.pdf</code>.
      </p>
      <h2>Limits</h2>
      <ul>
        <li>OCR reads Latin script only. Its models are ocrs (CC-BY-SA 4.0, Robert Knight).</li>
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
