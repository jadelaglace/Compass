// Compass Service Worker — PWA offline support
const CACHE_VERSION = 'v1';
const STATIC_CACHE = `compass-static-${CACHE_VERSION}`;
const API_CACHE = `compass-api-${CACHE_VERSION}`;

// Static assets to precache
const PRECACHE_ASSETS = [
  '/',
  '/index.html',
  '/manifest.json',
];

// ─── Install: precache static assets ───────────────────────────────────────
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(STATIC_CACHE)
      .then(cache => cache.addAll(PRECACHE_ASSETS))
      .then(() => self.skipWaiting())
  );
});

// ─── Activate: clean old caches ─────────────────────────────────────────────
self.addEventListener('activate', (event) => {
  const validCaches = [STATIC_CACHE, API_CACHE];
  event.waitUntil(
    caches.keys().then(cacheNames =>
      Promise.all(
        cacheNames
          .filter(name => !validCaches.includes(name))
          .map(name => caches.delete(name))
      )
    ).then(() => self.clients.claim())
  );
});

// ─── Fetch: network-first for API, cache-first for static ───────────────────
self.addEventListener('fetch', (event) => {
  const { request } = event;
  const url = new URL(request.url);

  // Skip non-GET and chrome-extension requests
  if (request.method !== 'GET') return;
  if (url.protocol === 'chrome-extension:') return;

  // API requests: network-first with cache fallback
  if (url.pathname.startsWith('/api') || url.pathname.startsWith('/entities') || url.pathname.startsWith('/graph') || url.pathname.startsWith('/insights') || url.pathname.startsWith('/feed') || url.pathname.startsWith('/search') || url.pathname.startsWith('/decay')) {
    event.respondWith(
      fetch(request)
        .then(response => {
          if (response.ok) {
            const clone = response.clone();
            caches.open(API_CACHE).then(cache => cache.put(request, clone));
          }
          return response;
        })
        .catch(() => caches.match(request).then(cached => cached || offlineResponse('/offline')))
    );
    return;
  }

  // Static assets: cache-first
  event.respondWith(
    caches.match(request).then(cached => {
      if (cached) return cached;
      return fetch(request).then(response => {
        if (response.ok && (url.pathname.endsWith('.js') || url.pathname.endsWith('.css') || url.pathname.endsWith('.woff2') || url.pathname.endsWith('.png') || url.pathname.endsWith('.svg'))) {
          const clone = response.clone();
          caches.open(STATIC_CACHE).then(cache => cache.put(request, clone));
        }
        return response;
      });
    })
  );
});

// ─── Offline response helper ─────────────────────────────────────────────────
function offlineResponse(offlinePath) {
  return caches.match(offlinePath).then(html => {
    if (html) return html;
    return new Response('<html><body><h1>Offline</h1><p>You are currently offline. Please check your connection.</p></body></html>', {
      headers: { 'Content-Type': 'text/html' },
      status: 503,
    });
  });
}

// ─── Background sync: queue score / reference actions ───────────────────────
self.addEventListener('sync', (event) => {
  if (event.tag === 'compass-entity-sync') {
    event.waitUntil(syncPendingActions());
  }
});

async function syncPendingActions() {
  // Open IndexedDB and replay pending writes
  // This is a stub — full implementation uses idb-keyval or similar
  console.log('[SW] Syncing pending actions...');
  const clients = await self.clients.matchAll();
  clients.forEach(client => client.postMessage({ type: 'SYNC_COMPLETE' }));
}