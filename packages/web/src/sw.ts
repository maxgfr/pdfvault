/// <reference lib="webworker" />
// Offline support. The build replaces __MANIFEST__ with the hashed list of
// files: the app shell is cached at install; the page renderer and the OCR
// models are cached the first time a tool needs them.

const sw = self as unknown as ServiceWorkerGlobalScope;
declare const __MANIFEST__: { version: string; precache: string[]; lazy: string[] };

const { version, precache, lazy } = __MANIFEST__;
const CACHE = `pdfvault-${version}`;
const scope = new URL(sw.registration.scope);
const abs = (p: string) => new URL(p, scope).href;
const lazySet = new Set(lazy.map(abs));
// Files shared to the app wait here until the page takes them (scan/session.ts).
const INBOX = "pdfvault-inbox";
const SHARE_TARGET = abs("./share-target");

sw.addEventListener("install", (event: ExtendableEvent) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((c) => c.addAll(precache.map(abs)))
      .then(() => sw.skipWaiting()),
  );
});

sw.addEventListener("activate", (event: ExtendableEvent) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k.startsWith("pdfvault-") && k !== CACHE && k !== INBOX).map((k) => caches.delete(k))))
      .then(() => sw.clients.claim()),
  );
});

/**
 * Another app shared files to pdfvault (the manifest's share_target). They
 * never leave the device: they are parked in a cache the scanner page empties
 * as soon as it opens.
 */
async function receiveShared(req: Request): Promise<Response> {
  const form = await req.formData();
  await caches.delete(INBOX);
  const inbox = await caches.open(INBOX);
  const files = form.getAll("files").filter((f): f is File => f instanceof File);
  await Promise.all(
    files.map((f, i) =>
      inbox.put(abs(`./share-inbox/${i}`), new Response(f, { headers: { "content-type": f.type, "x-name": encodeURIComponent(f.name) } })),
    ),
  );
  return Response.redirect(abs("./#/scan?shared"), 303);
}

sw.addEventListener("fetch", (event: FetchEvent) => {
  const req = event.request;
  const url = new URL(req.url);
  if (req.method === "POST" && url.origin + url.pathname === SHARE_TARGET) {
    event.respondWith(receiveShared(req));
    return;
  }
  if (req.method !== "GET" || url.origin !== scope.origin || !url.pathname.startsWith(scope.pathname)) return;
  // Navigations always get the app shell: routing lives in the hash.
  const key = req.mode === "navigate" ? abs("./") : url.origin + url.pathname;
  event.respondWith(
    (async () => {
      const cache = await caches.open(CACHE);
      const hit = await cache.match(key);
      if (hit) return hit;
      const res = await fetch(req);
      if (res.ok && (lazySet.has(key) || key === abs("./"))) await cache.put(key, res.clone());
      return res;
    })(),
  );
});
