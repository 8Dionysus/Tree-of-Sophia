/** The public beacon token is deployment configuration, not a credential. */
export function withWebAnalytics(request: Request, response: Response, token: string): Response {
  const host = new URL(request.url).hostname.toLowerCase();
  if (host !== "treeofsophia.com" || request.method !== "GET" || !response.ok
      || !response.headers.get("Content-Type")?.toLowerCase().startsWith("text/html")
      || !/^[a-f0-9]{32}$/.test(token)) return response;

  // Preserve the existing Cloudflare "exclude EU visitors" setting.
  // HTML is personalized by geography; never let downstream caches share it.
  const headers = new Headers(response.headers);
  headers.set("Cache-Control", "private, no-store, no-transform");
  headers.delete("ETag");
  headers.delete("Content-Length");
  const html = new Response(response.body, { status: response.status, headers });
  if (request.cf?.isEUCountry === "1") return html;

  let present = false;
  return new HTMLRewriter()
    .on('script[src*="static.cloudflareinsights.com/beacon.min.js"]', {
      element() { present = true; },
    })
    .on("body", {
      element(element) {
        element.onEndTag((tag) => {
          if (!present) tag.before(
            `<script type="module" crossorigin="anonymous" src="https://static.cloudflareinsights.com/beacon.min.js" data-cf-beacon='${JSON.stringify({ token, version: "2026.10.0", r: 1, spa: 2 })}'></script>`,
            { html: true },
          );
        });
      },
    })
    .transform(html);
}
