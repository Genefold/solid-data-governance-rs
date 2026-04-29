import { assertEquals } from "@std/assert";
import { Router } from "./router.ts";

Deno.test("matches static route", () => {
  const r = new Router(() => {});
  let hit = false;
  r.add("/health", () => {
    hit = true;
  });
  const fakeRoot = { innerHTML: "" } as unknown as HTMLElement;
  r.render(fakeRoot, "/health");
  assertEquals(hit, true);
});

Deno.test("matches parameterised route", async () => {
  const r = new Router(() => {});
  let captured: Record<string, string> = {};
  r.add("/catalog/:org/:dataset", (_root, params) => {
    captured = params;
  });
  const fakeRoot = { innerHTML: "" } as unknown as HTMLElement;
  await r.render(fakeRoot, "/catalog/org-a/bert-v2");
  assertEquals(captured, { org: "org-a", dataset: "bert-v2" });
});

Deno.test("falls through to notFound", async () => {
  let nf = false;
  const r = new Router(() => {
    nf = true;
  });
  r.add("/", () => {});
  const fakeRoot = { innerHTML: "" } as unknown as HTMLElement;
  await r.render(fakeRoot, "/missing");
  assertEquals(nf, true);
});
