/// Live-reload injection script (verbatim from `zlp-src/server.js:251-276`).
pub const INJECT_SCRIPT: &str = r#"
<!-- __ZED_LIVE_PREVIEW_INJECT__ -->
<script>
(() => {
  try {
    const sseUrl = '/__sse?target=' + encodeURIComponent(location.pathname);
    const es = new EventSource(sseUrl);
    es.onmessage = (e) => {
      if (e.data === 'reload') {
        location.reload();
      }
    };
    es.addEventListener('css', () => {
      const links = document.querySelectorAll('link[rel="stylesheet"]');
      for (const link of links) {
        const u = new URL(link.href, location.origin);
        u.searchParams.set('_lp_t', Date.now());
        link.href = u.href;
      }
    });
  } catch (err) {
    console.warn('[LivePreview] SSE error:', err);
  }
})();
</script>
"#;
