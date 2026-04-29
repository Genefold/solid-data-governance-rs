// Entry point for the governance console SPA.
//
// Pure-TypeScript bootstrap: wire the router, hook click + popstate
// for in-app navigation, then render the current path.

import { Router } from "./router.ts";
import { catalogView } from "./views/catalog_view.ts";
import { datasetView } from "./views/dataset_view.ts";
import { healthView } from "./views/health_view.ts";

const router = new Router((root) => {
  root.innerHTML = `<div class="card error">404 — no route matched.</div>`;
});

router
  .add("/", catalogView)
  .add("/health", healthView)
  .add("/catalog/:org/:dataset", datasetView);

const root = document.getElementById("app");
if (!root) throw new Error("missing #app root element");

addEventListener("popstate", () => router.render(root, location.pathname));

document.addEventListener("click", (event) => {
  const target = event.target as HTMLElement | null;
  if (!target) return;
  const link = target.closest("a[data-nav]") as HTMLAnchorElement | null;
  if (!link) return;
  const href = link.getAttribute("href") ?? "/";
  if (href.startsWith("http")) return;
  event.preventDefault();
  history.pushState({}, "", href);
  router.render(root, href);
});

router.render(root, location.pathname);
