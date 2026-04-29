// Tiny path-pattern router. No external dependencies.
//
// A route is a path template like `/catalog/:org/:dataset` whose handler
// receives the parsed parameter map. The first matching route wins.

export type RouteParams = Record<string, string>;
export type RouteHandler = (root: HTMLElement, params: RouteParams) => void | Promise<void>;

interface Route {
  pattern: string;
  segments: { name: string; param: boolean }[];
  handler: RouteHandler;
}

export class Router {
  private routes: Route[] = [];
  private notFound: RouteHandler;

  constructor(notFound: RouteHandler) {
    this.notFound = notFound;
  }

  add(pattern: string, handler: RouteHandler): this {
    const segments = pattern
      .split("/")
      .filter((s) => s.length > 0)
      .map((s) => {
        if (s.startsWith(":")) return { name: s.slice(1), param: true };
        return { name: s, param: false };
      });
    this.routes.push({ pattern, segments, handler });
    return this;
  }

  match(path: string): { handler: RouteHandler; params: RouteParams } {
    const parts = path.split("/").filter((s) => s.length > 0);
    for (const route of this.routes) {
      if (route.segments.length !== parts.length) continue;
      const params: RouteParams = {};
      let ok = true;
      for (let i = 0; i < parts.length; i++) {
        const seg = route.segments[i];
        if (seg.param) {
          params[seg.name] = decodeURIComponent(parts[i]);
        } else if (seg.name !== parts[i]) {
          ok = false;
          break;
        }
      }
      if (ok) return { handler: route.handler, params };
    }
    return { handler: this.notFound, params: {} };
  }

  async render(root: HTMLElement, path: string): Promise<void> {
    const { handler, params } = this.match(path);
    try {
      await handler(root, params);
    } catch (err) {
      console.error(err);
      root.innerHTML =
        `<div class="card error">Route render failed: ${escapeHtml(String(err))}</div>`;
    }
  }
}

export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}
