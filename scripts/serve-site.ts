// Serves the website in site/ on this computer, for looking at changes:
//   bun run site          http://127.0.0.1:8000
//   PORT=8080 bun run site
// Files are sent with caching turned off, so a normal reload shows every edit.
import { join, normalize, resolve, sep } from "node:path";

const root = resolve(import.meta.dir, "..", "site");
const port = Number(process.env.PORT ?? 8000);

const server = Bun.serve({
  hostname: "127.0.0.1",
  port,
  async fetch(request) {
    const url = new URL(request.url);
    let path = decodeURIComponent(url.pathname);
    if (path.endsWith("/")) path += "index.html";
    const file = normalize(join(root, path));
    // Nothing outside site/ is served.
    if (file !== root && !file.startsWith(root + sep)) {
      return new Response("Not found", { status: 404 });
    }
    let target = Bun.file(file);
    if (!(await target.exists())) {
      // `/docs` finds docs.html, as on the published site.
      target = Bun.file(file + ".html");
      if (!(await target.exists())) return new Response("Not found", { status: 404 });
    }
    return new Response(target, { headers: { "Cache-Control": "no-store" } });
  },
});

console.log(`Website: http://127.0.0.1:${server.port}/`);
console.log(`Docs:    http://127.0.0.1:${server.port}/docs.html`);
console.log("Stop with Ctrl+C.");
