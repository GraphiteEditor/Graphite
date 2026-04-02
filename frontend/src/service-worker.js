// These placeholders are replaced in `vite.config.ts` at build time
const PRECACHE_MANIFEST = self.__PRECACHE_MANIFEST;
const DEFERRED_CACHE_MANIFEST = self.__DEFERRED_CACHE_MANIFEST;
const SERVICE_WORKER_CONTENT_HASH = self.__SERVICE_WORKER_CONTENT_HASH;

const STATIC_CACHE_NAME = `static-${SERVICE_WORKER_CONTENT_HASH}`;
const RUNTIME_ASSETS = "runtime-assets";
const RUNTIME_FONTS = "runtime-fonts";

const FONT_LIST_API = "https://api.graphite.art/font-list";

// Build a set of precache URLs for quick lookup during fetch
const PRECACHE_URLS = new Set(PRECACHE_MANIFEST.map((entry) => new URL(entry.url, self.location.origin).href));

// Track deferred manifest URLs and revisions for cache invalidation
const DEFERRED_ENTRIES = new Map(DEFERRED_CACHE_MANIFEST.map((entry) => [new URL(entry.url, self.location.origin).href, entry.revision]));

// ==================
// Caching strategies
// ==================

function isCacheable(response) {
	// Cache normal successful responses and opaque responses (cross-origin no-cors, e.g. <link> stylesheets)
	return response.ok || response.type === "opaque";
}

async function cacheFirst(request, cacheName) {
	const cache = await caches.open(cacheName);
	const cached = await cache.match(request);
	if (cached) return cached;

	const response = await fetch(request);
	if (isCacheable(response)) cache.put(request, response.clone());
	return response;
}

async function networkFirst(request, cacheName) {
	const cache = await caches.open(cacheName);
	try {
		const response = await fetch(request);
		if (isCacheable(response)) cache.put(request, response.clone());
		return response;
	} catch {
		const cached = await cache.match(request);
		if (cached) return cached;
		throw new Error(`Network request failed and no cache available for ${request.url}`);
	}
}

// ================
// Lifecycle events
// ================

self.addEventListener("install", (event) => {
	event.waitUntil(
		(async () => {
			// Precache app shell assets
			const cache = await caches.open(STATIC_CACHE_NAME);
			await cache.addAll(PRECACHE_MANIFEST.map((entry) => entry.url));

			// Proactively cache the font catalog API
			try {
				const fontResponse = await fetch(FONT_LIST_API);
				if (fontResponse.ok) {
					const fontCache = await caches.open(RUNTIME_FONTS);
					await fontCache.put(FONT_LIST_API, fontResponse);
				}
			} catch {
				// Font catalog prefetch is best-effort, don't block installation
			}

			await self.skipWaiting();
		})(),
	);
});

self.addEventListener("activate", (event) => {
	event.waitUntil(
		(async () => {
			const cacheNames = await caches.keys();

			// Delete old precache versions
			const deletions = cacheNames.filter((name) => name.startsWith("static-") && name !== STATIC_CACHE_NAME).map((name) => caches.delete(name));
			await Promise.all(deletions);

			// Prune stale deferred (demo artwork) entries
			const assetsCache = await caches.open(RUNTIME_ASSETS);
			const assetsKeys = await assetsCache.keys();
			await Promise.all(assetsKeys.filter((request) => !DEFERRED_ENTRIES.has(request.url)).map((request) => assetsCache.delete(request)));

			await self.clients.claim();
		})(),
	);
});

// =============
// Fetch routing
// =============

self.addEventListener("fetch", (event) => {
	const { request } = event;
	const url = new URL(request.url);

	// Pass through range requests (e.g. for large file streaming) and non-GET requests
	if (request.headers.has("range") || request.method !== "GET") return;

	// Pre-cached assets (JS and Wasm bundle files, favicons, index.html)
	if (PRECACHE_URLS.has(url.href)) {
		event.respondWith(cacheFirst(request, STATIC_CACHE_NAME));
		return;
	}

	// Deferred-cached assets (demo artwork, third-party licenses, etc.)
	if (DEFERRED_ENTRIES.has(url.href)) {
		event.respondWith(cacheFirst(request, RUNTIME_ASSETS));
		return;
	}

	// Font catalog API: network-first to keep it fresh
	if (url.href.startsWith(FONT_LIST_API)) {
		event.respondWith(networkFirst(request, RUNTIME_FONTS));
		return;
	}

	// Google Fonts CSS (font preview stylesheets): cache-first since responses are stable for a given query
	if (url.hostname === "fonts.googleapis.com") {
		event.respondWith(cacheFirst(request, RUNTIME_FONTS));
		return;
	}

	// Google Fonts static files: cache-first since they are immutable CDN URLs
	if (url.hostname === "fonts.gstatic.com") {
		event.respondWith(cacheFirst(request, RUNTIME_FONTS));
		return;
	}

	// Navigation requests: serve cached index.html
	if (request.mode === "navigate") {
		event.respondWith(cacheFirst(request, STATIC_CACHE_NAME));
		return;
	}

	// Everything else: network-only (no respondWith, let the browser handle it)
});

// ============================
// Deferred caching via message
// ============================

self.addEventListener("message", (event) => {
	if (event.data?.type !== "CACHE_DEFERRED") return;

	event.waitUntil(
		(async () => {
			const cache = await caches.open(RUNTIME_ASSETS);
			const fetchPromises = DEFERRED_CACHE_MANIFEST.map(async (entry) => {
				const fullUrl = new URL(entry.url, self.location.origin).href;

				// Skip if already cached with the same revision
				const existing = await cache.match(fullUrl);
				if (existing) return;

				try {
					const response = await fetch(fullUrl);
					if (response.ok) await cache.put(fullUrl, response);
				} catch {
					// Best-effort: skip files that fail to fetch
				}
			});
			await Promise.all(fetchPromises);
		})(),
	);
});
