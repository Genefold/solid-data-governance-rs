// Build script: bundle src/main.ts into static/main.js using Deno's
// built-in emit. Pure TypeScript, no third-party bundlers.
//
// Run: `deno task build`.

import { dirname, fromFileUrl, join } from "@std/path";

const here = dirname(fromFileUrl(import.meta.url));
const projectRoot = join(here, "..");
const entry = join(projectRoot, "src", "main.ts");
const out = join(projectRoot, "static", "main.js");

const result = await new Deno.Command(Deno.execPath(), {
  args: ["bundle", "--platform", "browser", "-o", out, entry],
  stdout: "inherit",
  stderr: "inherit",
}).output();

if (!result.success) {
  console.error("bundle failed");
  Deno.exit(1);
}

console.log(`built ${out}`);
