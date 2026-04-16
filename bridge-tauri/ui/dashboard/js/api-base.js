/**
 * RKStratum Bridge desktop: operator UI is served from the Tauri asset bundle while JSON APIs
 * stay on the in-process bridge HTTP listener. Pass `?api=http%3A%2F%2F127.0.0.1%3APORT` so fetches
 * hit the correct origin (CORS is `*` on bridge JSON).
 */
(function () {
  const params = new URLSearchParams(window.location.search);
  const api = params.get('api');
  window.__RKSTRATUM_API_ORIGIN__ = api ? api.replace(/\/$/, '') : '';
})();

/**
 * @param {string} path e.g. "api/status"
 */
function rkstratumApiUrl(path) {
  const p = String(path || '').replace(/^\//, '');
  const b = window.__RKSTRATUM_API_ORIGIN__;
  if (!b) return p;
  return b + '/' + p;
}

function rkstratumPatchNavLinks() {
  const api = new URLSearchParams(location.search).get('api');
  if (!api) return;
  const q = 'api=' + encodeURIComponent(api);
  document.querySelectorAll('a[href]').forEach((a) => {
    const href = a.getAttribute('href');
    if (!href || href.startsWith('http') || href.startsWith('mailto:') || href.startsWith('#')) return;
    if (href.includes('api=')) return;
    const [pathPart, hash] = href.split('#');
    const sep = pathPart.includes('?') ? '&' : '?';
    a.setAttribute('href', pathPart + sep + q + (hash ? '#' + hash : ''));
  });
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', rkstratumPatchNavLinks);
} else {
  rkstratumPatchNavLinks();
}
