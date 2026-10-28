import { createRoot } from "react-dom/client";
import { App } from "./app.tsx";
import { scriptURL } from "./trusted.ts";
import { readFiles, workspace } from "./workspace.ts";

// GitHub Pages cannot send frame-ancestors; refuse to run inside a frame.
if (window.top !== window.self) {
  document.documentElement.replaceChildren();
  throw new Error("pdfvault does not run inside frames");
}

const root = document.getElementById("root");
if (root) createRoot(root).render(<App />);

// Files opened with the installed app from the system (the manifest's file_handlers) join your files.
const launchQueue = (window as { launchQueue?: { setConsumer(fn: (p: { files: FileSystemFileHandle[] }) => void): void } }).launchQueue;
launchQueue?.setConsumer(async ({ files }) => {
  if (!files.length) return;
  workspace.add(await readFiles(await Promise.all(files.map((h) => h.getFile()))));
});

const secure = location.protocol === "https:" || location.hostname === "localhost" || location.hostname === "127.0.0.1";
if ("serviceWorker" in navigator && secure) {
  void navigator.serviceWorker.register(scriptURL("./sw.js"), { scope: "./" }).catch(() => {});
}
