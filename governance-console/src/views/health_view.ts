import { escapeHtml } from "../router.ts";

export async function healthView(root: HTMLElement): Promise<void> {
  root.innerHTML = `<div class="card">Probing server…</div>`;
  let summary = "";
  try {
    const res = await fetch("/", { headers: { "Accept": "text/turtle" } });
    summary =
      `LDP root status: <strong>${res.status}</strong>` +
      `<br/>Link: <code>${escapeHtml(res.headers.get("Link") ?? "(none)")}</code>`;
  } catch (err) {
    summary = `<span class="error">${escapeHtml(String(err))}</span>`;
  }
  let catalogStatus = "";
  try {
    const res = await fetch("/catalog");
    catalogStatus =
      `Catalog status: <strong>${res.status}</strong> (${res.headers.get("Content-Type") ?? ""})`;
  } catch (err) {
    catalogStatus = `<span class="error">${escapeHtml(String(err))}</span>`;
  }

  root.innerHTML = `
    <h2>Server health</h2>
    <div class="card">${summary}</div>
    <div class="card">${catalogStatus}</div>
  `;
}
