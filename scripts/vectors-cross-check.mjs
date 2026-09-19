// vectors-cross-check.mjs — the cross-language corpus identity check
// (TODO 234): the golden vectors unidpp-ts and unidpp-rb vendor must
// be byte-identical to the canonical fixtures they were vendored
// from (unidpp-core's crates and unidpp-signatif). The per-repo
// manifests pin the digests; this script recomputes the originals
// in-family and compares all three.

import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const FAMILY = process.env.UNIDPP_FAMILY_DIR ?? path.join(here, "..", "..");

const SOURCES = [
  "unidpp-core/crates/s13/fixtures/canonical",
  "unidpp-core/crates/grid/fixtures/canonical",
  "unidpp-core/crates/semantics/fixtures/canonical",
  "unidpp-signatif/fixtures/canonical",
];
const VENDORS = [
  ["unidpp-ts", "test-vectors"],
  ["unidpp-rb", "spec/fixtures/vectors"],
];

function sha256(data) {
  return createHash("sha256").update(data).digest("hex");
}

// The originals have no manifest of their own; derive the file list
// from the vendored manifests and verify each file against its
// original bytes.
let failed = 0;
for (const name of (await readFile(path.join(FAMILY, VENDORS[0][0], VENDORS[0][1], "vectors.sha256"), "utf8"))
  .trim()
  .split("\n")
  .map((line) => line.split(/\s+/)[1])) {
  let found = null;
  for (const source of SOURCES) {
    const bytes = await readFile(path.join(FAMILY, source, name)).catch(() => null);
    if (bytes) {
      found = { source, digest: sha256(bytes) };
      break;
    }
  }
  if (!found) {
    console.log(`vectors-cross-check: FAIL ${name} — no original carries it`);
    failed = 1;
    continue;
  }
  for (const [repo, dir] of VENDORS) {
    const vendored = await readFile(path.join(FAMILY, repo, dir, name));
    const manifest = await readFile(path.join(FAMILY, repo, dir, "vectors.sha256"), "utf8");
    const pinned = manifest
      .trim()
      .split("\n")
      .map((line) => line.split(/\s+/))
      .find(([, n]) => n === name)[0];
    if (sha256(vendored) !== found.digest || pinned !== found.digest) {
      console.log(`vectors-cross-check: FAIL ${name} — ${repo} disagrees with ${found.source}`);
      failed = 1;
    }
  }
}
console.log(
  failed === 0
    ? `vectors-cross-check: ok — the vendored corpora are byte-identical to the originals`
    : "vectors-cross-check: FAIL",
);
process.exit(failed);
