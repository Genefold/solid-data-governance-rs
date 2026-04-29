import { api, type CatalogEntry } from "../api/client.ts";
import { escapeHtml } from "../router.ts";

export async function catalogView(root: HTMLElement): Promise<void> {
  root.innerHTML = `<div class="card">Loading catalog…</div>`;
  let datasets: CatalogEntry[] = [];
  let error: string | null = null;
  try {
    const res = await api.listDatasets();
    datasets = res.datasets ?? [];
  } catch (err) {
    error = String(err);
  }

  root.innerHTML = `
    <h2>Catalog</h2>
    <div class="card">
      <h3 style="margin-top:0">Register a dataset</h3>
      <form id="reg-form">
        <label for="org">Org</label>
        <input id="org" name="org" placeholder="org-a" required />
        <label for="dataset">Dataset id</label>
        <input id="dataset" name="dataset" placeholder="bert-v2" required />
        <label for="title">Title</label>
        <input id="title" name="title" placeholder="BERT v2 embeddings" required />
        <label for="description">Description</label>
        <input id="description" name="description" />
        <label for="shape">Shape (comma-separated)</label>
        <input id="shape" name="shape" value="0" />
        <label for="dtype">Dtype</label>
        <input id="dtype" name="dtype" value="float32" />
        <div style="margin-top:14px">
          <button type="submit">Register</button>
        </div>
      </form>
      <p id="reg-status" class="muted" style="margin-top:10px"></p>
    </div>
    <div class="card">
      ${error ? `<div class="error">${escapeHtml(error)}</div>` : renderTable(datasets)}
    </div>
  `;

  const form = root.querySelector<HTMLFormElement>("#reg-form")!;
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const data = new FormData(form);
    const org = String(data.get("org") ?? "").trim();
    const dataset = String(data.get("dataset") ?? "").trim();
    const shape = String(data.get("shape") ?? "")
      .split(",")
      .map((s) => Number(s.trim()))
      .filter((n) => !Number.isNaN(n));
    const status = root.querySelector<HTMLElement>("#reg-status")!;
    status.textContent = "Submitting…";
    try {
      await api.createDataset(org, dataset, {
        title: String(data.get("title") ?? ""),
        description: String(data.get("description") ?? ""),
        shape,
        dtype: String(data.get("dtype") ?? "float32"),
      });
      status.textContent = "Registered. Reloading…";
      // Trigger re-render via popstate.
      history.pushState({}, "", `/catalog/${encodeURIComponent(org)}/${encodeURIComponent(dataset)}`);
      dispatchEvent(new PopStateEvent("popstate"));
    } catch (err) {
      status.innerHTML = `<span class="error">${escapeHtml(String(err))}</span>`;
    }
  });
}

function renderTable(datasets: CatalogEntry[]): string {
  if (datasets.length === 0) {
    return `<p class="muted">No datasets registered yet.</p>`;
  }
  const rows = datasets.map((d) => {
    const tier = d.policy?.default_tier ?? "discovery";
    const [org, name] = d.dataset_id.split("/", 2);
    return `
      <tr>
        <td><a data-nav href="/catalog/${encodeURIComponent(org)}/${encodeURIComponent(name)}">${
      escapeHtml(d.dataset_id)
    }</a></td>
        <td>${escapeHtml(d.title)}</td>
        <td><span class="badge ${tier}">${tier}</span></td>
        <td class="muted">${escapeHtml(d.created_at)}</td>
      </tr>
    `;
  }).join("");
  return `
    <table>
      <thead><tr><th>Dataset</th><th>Title</th><th>Default tier</th><th>Created</th></tr></thead>
      <tbody>${rows}</tbody>
    </table>
  `;
}
