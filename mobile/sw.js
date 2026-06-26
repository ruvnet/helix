// Helix PWA service worker — cache-first app shell so the console works offline
// (local-first is the whole point: ADR-001/013). Bump CACHE on asset changes.
const CACHE = "helix-v1";
const SHELL = [
  "./index.html",
  "./mobile.css",
  "./mobile.js",
  "./manifest.webmanifest",
  "./icon.svg",
  "../ui/pkg/helix.js",
  "../ui/pkg/helix_bg.wasm",
];

self.addEventListener("install", (e) => {
  e.waitUntil(caches.open(CACHE).then((c) => c.addAll(SHELL)).then(() => self.skipWaiting()));
});

self.addEventListener("activate", (e) => {
  e.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k)))
    ).then(() => self.clients.claim())
  );
});

self.addEventListener("fetch", (e) => {
  if (e.request.method !== "GET") return;
  e.respondWith(
    caches.match(e.request).then((hit) =>
      hit ||
      fetch(e.request).then((res) => {
        const copy = res.clone();
        caches.open(CACHE).then((c) => c.put(e.request, copy)).catch(() => {});
        return res;
      }).catch(() => hit)
    )
  );
});
