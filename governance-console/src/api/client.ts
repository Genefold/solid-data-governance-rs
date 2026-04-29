// Lightweight typed API client for the governance plane.
//
// All calls are JSON in / JSON out. The API origin is configured at
// runtime via `globalThis.__API_BASE__`, falling back to the current
// origin so the console can be served by the Rust server itself.

export interface CatalogEntry {
  dataset_id: string;
  title: string;
  description: string;
  created_at: string;
  policy: AccessPolicy | null;
}

export type Tier = "discovery" | "evaluation" | "training" | "inference";

export interface AccessRule {
  principal: string;
  tier: Tier;
  expires_at?: string | null;
  byte_cap?: number | null;
}

export interface AccessPolicy {
  dataset_id: string;
  default_tier: Tier;
  rules: AccessRule[];
}

export interface AuditEvent {
  timestamp: string;
  event_id: string;
  action: string;
  dataset_id: string;
  principal?: string;
  tier?: string;
  bytes?: number;
  resource?: string;
  status?: number;
}

export interface IssueTokenInput {
  webid: string;
  tier: Tier;
  ttl_seconds?: number;
  byte_cap?: number;
  dpop_jkt: string;
}

export interface IssueTokenResponse {
  token: string;
  claims: {
    sub: string;
    webid: string;
    dataset_id: string;
    access_tier: Tier;
    exp: number;
    iat: number;
    cnf: { jkt: string };
  };
}

declare global {
  // eslint-disable-next-line no-var
  var __API_BASE__: string | undefined;
}

function base(): string {
  if (typeof globalThis.__API_BASE__ === "string") return globalThis.__API_BASE__;
  if (typeof location !== "undefined") return location.origin;
  return "http://localhost:3000";
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(`${base()}${path}`, {
    headers: { "Accept": "application/json", ...(init.headers ?? {}) },
    ...init,
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`${res.status} ${res.statusText}: ${text}`);
  }
  if (res.status === 204) return undefined as T;
  const ct = res.headers.get("Content-Type") ?? "";
  if (ct.includes("application/json")) return await res.json() as T;
  return await res.text() as unknown as T;
}

export interface CatalogList {
  datasets: CatalogEntry[];
}

export const api = {
  listDatasets(): Promise<CatalogList> {
    return request<CatalogList>("/catalog");
  },

  getDataset(org: string, dataset: string): Promise<CatalogEntry> {
    return request<CatalogEntry>(`/catalog/${encodeURIComponent(org)}/${encodeURIComponent(dataset)}`);
  },

  createDataset(
    org: string,
    dataset: string,
    body: {
      title: string;
      description?: string;
      shape: number[];
      chunk_shape?: number[];
      dtype?: string;
    },
  ): Promise<CatalogEntry> {
    return request<CatalogEntry>(
      `/catalog/${encodeURIComponent(org)}/${encodeURIComponent(dataset)}`,
      {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      },
    );
  },

  putPolicy(org: string, dataset: string, policy: AccessPolicy): Promise<void> {
    return request<void>(
      `/catalog/${encodeURIComponent(org)}/${encodeURIComponent(dataset)}/policy`,
      {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(policy),
      },
    );
  },

  issueToken(
    org: string,
    dataset: string,
    input: IssueTokenInput,
  ): Promise<IssueTokenResponse> {
    return request<IssueTokenResponse>(
      `/catalog/${encodeURIComponent(org)}/${encodeURIComponent(dataset)}/tokens`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(input),
      },
    );
  },

  async getAudit(org: string, dataset: string): Promise<AuditEvent[]> {
    const res = await fetch(
      `${base()}/catalog/${encodeURIComponent(org)}/${encodeURIComponent(dataset)}/audit`,
    );
    if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
    const text = await res.text();
    return text
      .split("\n")
      .filter((l) => l.trim().length > 0)
      .map((l) => JSON.parse(l) as AuditEvent);
  },
};
