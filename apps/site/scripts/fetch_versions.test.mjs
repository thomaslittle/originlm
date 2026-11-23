import test from "node:test";
import assert from "node:assert/strict";
import { compareVersions, resolveVersions } from "./fetch_versions.mjs";

test("compareVersions orders SemVer and PEP 440 pre-releases accurately", () => {
  assert.ok(compareVersions("0.1.0-alpha.29", "0.1.0-alpha.28") > 0);
  assert.ok(compareVersions("0.1.0a29", "0.1.0a28") > 0);
  assert.equal(compareVersions("0.1.0-alpha.29", "0.1.0a29"), 0);
  assert.ok(compareVersions("0.1.0", "0.1.0-alpha.29") > 0);
  assert.ok(compareVersions("0.1.0-alpha.25", "0.1.0-alpha.29") < 0);
});

test("resolveVersions uses live registry versions when repo is in sync or behind", () => {
  const { versions, sources } = resolveVersions(
    {
      npm: { ok: true, version: "0.1.0-alpha.29" },
      pypi: { ok: true, version: "0.1.0a29" },
      crates: { ok: true, version: "0.1.0-alpha.29" },
    },
    { npm: "0.1.0-alpha.29", pypi: "0.1.0a29", crates: "0.1.0-alpha.29" },
  );
  assert.deepEqual(versions, {
    npm: "0.1.0-alpha.29",
    pypi: "0.1.0a29",
    crates: "0.1.0-alpha.29",
  });
  assert.deepEqual(sources, { npm: "registry", pypi: "registry", crates: "registry" });
});

test("resolveVersions falls back to repo version files when a registry fetch fails", () => {
  const warnings = [];
  const { versions, sources } = resolveVersions(
    {
      npm: { ok: true, version: "0.1.0-alpha.29" },
      pypi: { ok: false, error: "network timeout" },
      crates: { ok: true, version: "0.1.0-alpha.29" },
    },
    { npm: "0.1.0-alpha.29", pypi: "0.1.0a29", crates: "0.1.0-alpha.29" },
    (msg) => warnings.push(msg),
  );
  assert.equal(versions.pypi, "0.1.0a29");
  assert.equal(sources.pypi, "repo-fallback");
  assert.equal(warnings.length, 1);
});

test("resolveVersions fails loudly when repo version is ahead of published registry", () => {
  assert.throws(
    () =>
      resolveVersions(
        {
          npm: { ok: true, version: "0.1.0-alpha.29" },
          pypi: { ok: true, version: "0.1.0a29" },
          crates: { ok: true, version: "0.1.0-alpha.28" },
        },
        { npm: "0.1.0-alpha.29", pypi: "0.1.0a29", crates: "0.1.0-alpha.29" },
      ),
    /crates: repo version \(0\.1\.0-alpha\.29\) is ahead of published registry version \(0\.1\.0-alpha\.28\)/,
  );
});
