import { api, type AuditEvent, type CatalogEntry, type Tier } from "../api/client.ts";
import { escapeHtml } from "../router.ts";

export async function datasetView(
  root: HTMLElement,
  params: Record<string, string>,
): Promise<void> {
  const { org, dataset } = params;
  root.innerHTML = `<div class="card">Loading ${escapeHtml(`${org}/${dataset}`)}…</div>`;

  let entry: CatalogEntry | null = null;
  let audit: AuditEvent[] = [];
  let error: string | null = null;
  try {
    entry = await api.getDataset(org, dataset);
    audit = await api.getAudit(org, dataset);
  } catch (err) {
    error = String(err);
  }
  if (error || !entry) {
    root.innerHTML =
      `<div class="card error">Failed to load dataset: ${escapeHtml(error ?? "")}</div>`;
    return;
  }

  const tier = entry.policy?.default_tier ?? "discovery";

  root.innerHTML = `
    <p><a data-nav href="/">← Catalog</a></p>
    <h2>${escapeHtml(entry.dataset_id)}</h2>
    <div class="card">
      <p><strong>Title:</strong> ${escapeHtml(entry.title)}</p>
      <p><strong>Description:</strong> ${escapeHtml(entry.description ?? "")}</p>
      <p><strong>Default tier:</strong> <span class="badge ${tier}">${tier}</span></p>
      <p><strong>Created:</strong> <span class="muted">${escapeHtml(entry.created_at)}</span></p>
    </div>

    <div class="card">
      <h3 style="margin-top:0">Issue capability token</h3>
      <form id="token-form">
        <label for="webid">WebID</label>
        <input id="webid" name="webid" placeholder="https://alice.example/#me" required />
        <label for="tier">Tier</label>
        <select id="tier" name="tier">
          <option value="discovery">discovery</option>
          <option value="evaluation">evaluation</option>
          <option value="training" selected>training</option>
          <option value="inference">inference</option>
        </select>
        <label for="dpop_jkt">DPoP JKT</label>
        <input id="dpop_jkt" name="dpop_jkt" placeholder="<sha-256 thumbprint>" required />
        <label for="ttl">TTL (seconds)</label>
        <input id="ttl" name="ttl" type="number" value="3600" />
        <div style="margin-top:14px">
          <button type="submit">Mint token</button>
        </div>
      </form>
      <pre id="token-output" hidden></pre>
    </div>

    <div class="card">
      <h3 style="margin-top:0">Audit trail</h3>
      ${renderAudit(audit)}
    </div>
  `;

  const form = root.querySelector<HTMLFormElement>("#token-form")!;
  const out = root.querySelector<HTMLPreElement>("#token-output")!;
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const data = new FormData(form);
    out.hidden = false;
    out.textContent = "Minting…";
    try {
      const res = await api.issueToken(org, dataset, {
        webid: String(data.get("webid") ?? ""),
        tier: String(data.get("tier") ?? "training") as Tier,
        dpop_jkt: String(data.get("dpop_jkt") ?? ""),
        ttl_seconds: Number(data.get("ttl") ?? 3600),
      });
      out.textContent = JSON.stringify(res, null, 2);
    } catch (err) {
      out.innerHTML = `<span class="error">${escapeHtml(String(err))}</span>`;
    }
  });
}

function renderAudit(events: AuditEvent[]): string {
  if (events.length === 0) return `<p class="muted">No audit events yet.</p>`;
  const rows = events.slice().reverse().map((e) =>
    `<tr>
      <td class="muted">${escapeHtml(e.timestamp)}</td>
      <td>${escapeHtml(e.action)}</td>
      <td>${escapeHtml(e.principal ?? "")}</td>
      <td>${escapeHtml(e.tier ?? "")}</td>
    </tr>`
  ).join("");
  return `<table>
    <thead><tr><th>Timestamp</th><th>Action</th><th>Principal</th><th>Tier</th></tr></thead>
    <tbody>${rows}</tbody>
  </table>`;
}
