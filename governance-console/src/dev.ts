// Dev server for the governance console.
//
// Serves the static/ directory on port 3000 and proxies anything that
// looks like an API call (`/catalog`, `/datasets`) to the Rust server
// at API_BASE (default http://localhost:8080).

import { serveDir } from "@std/http/file-server";
import { dirname, fromFileUrl, join } from "@std/path";

const here = dirname(fromFileUrl(import.meta.url));
const staticRoot = join(here, "..", "static");

const PORT = Number(Deno.env.get("PORT") ?? "3000");
const API_BASE = Deno.env.get("API_BASE") ?? "http://localhost:8080";

const apiPrefixes = ["/catalog", "/datasets"];

function isApi(path: string): boolean {
  return apiPrefixes.some((p) => path === p || path.startsWith(p + "/"));
}

async function proxy(req: Request): Promise<Response> {
  const url = new URL(req.url);
  const target = `${API_BASE}${url.pathname}${url.search}`;
  const init: RequestInit = {
    method: req.method,
    headers: req.headers,
    body: ["GET", "HEAD"].includes(req.method) ? undefined : await req.arrayBuffer(),
  };
  return await fetch(target, init);
}

console.log(`Console dev server on http://localhost:${PORT}, proxying API → ${API_BASE}`);

Deno.serve({ port: PORT }, async (req) => {
  const url = new URL(req.url);
  if (isApi(url.pathname)) return await proxy(req);
  // SPA fallback: serve index.html for any non-asset path.
  if (
    !url.pathname.startsWith("/static/") &&
    !url.pathname.includes(".")
  ) {
    return await serveDir(
      new Request(`${url.origin}/index.html`, req),
      { fsRoot: staticRoot, urlRoot: "" },
    );
  }
  if (url.pathname.startsWith("/static/")) {
    return await serveDir(req, { fsRoot: staticRoot, urlRoot: "static" });
  }
  return await serveDir(req, { fsRoot: staticRoot, urlRoot: "" });
});
