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
      .then((keys) => Promise.all(keys.filter((k) => k.startsWith("pdfvault-") && k !== CACHE).map((k) => caches.delete(k))))
      .then(() => sw.clients.claim()),
  );
});

sw.addEventListener("fetch", (event: FetchEvent) => {
  const req = event.request;
  const url = new URL(req.url);
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
