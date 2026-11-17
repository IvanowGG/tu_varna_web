// Offline support: the app shell is cached; API calls always go to the network.
const VERSION = 'tuv-hub-v1';
const SHELL = ['/', '/css/app.css', '/js/main.js', '/js/ui.js', '/js/i18n.js', '/js/icons.js', '/js/api.js', '/js/store.js',
  '/js/md.js', '/js/captcha.js', '/js/pow-worker.js', '/js/timetable.js', '/js/views/dashboard.js', '/js/views/schedule.js',
  '/js/views/tasks.js', '/js/views/notes.js', '/js/views/grades.js', '/js/views/docs.js', '/js/views/focus.js',
  '/js/views/links.js', '/js/views/settings.js', '/js/views/profile.js', '/icon.svg', '/manifest.webmanifest'];

self.addEventListener('install', (e) => {
  e.waitUntil(caches.open(VERSION).then((c) => c.addAll(SHELL)).then(() => self.skipWaiting()));
});
self.addEventListener('activate', (e) => {
  e.waitUntil(caches.keys().then((keys) => Promise.all(keys.filter((k) => k !== VERSION).map((k) => caches.delete(k)))).then(() => self.clients.claim()));
});
self.addEventListener('fetch', (e) => {
  const url = new URL(e.request.url);
  if (e.request.method !== 'GET' || url.origin !== location.origin) return;
  if (url.pathname.startsWith('/api/') || url.pathname.startsWith('/auth/')) return;
  // Network first, cache as fallback, so updates arrive at once and offline still works.
  e.respondWith(fetch(e.request).then((res) => {
    if (res.ok) { const copy = res.clone(); caches.open(VERSION).then((c) => c.put(e.request, copy)); }
    return res;
  }).catch(() => caches.match(e.request).then((r) => r || caches.match('/'))));
});
