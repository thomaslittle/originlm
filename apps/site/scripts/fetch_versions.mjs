/**
 * Fetch live published OriginLM versions from npm, PyPI, and crates.io at build time
 * and emit src/data/versions.generated.ts.
 *
 * Guardrail:
 * - If a registry fetch succeeds and the repo's own version file is AHEAD of what
 *   the registry reports, fail the build loudly (missing publish).
 * - If a registry fetch fails (offline / timeout / 5xx), fall back to the repo's
 *   version file for that registry with a warning.
 */
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const SITE_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = path.resolve(SITE_DIR, "..", "..");
const OUTPUT_FILE = path.join(SITE_DIR, "src", "data", "versions.generated.ts");

const USER_AGENT = "originlm-site-build (https://originlm.pages.dev)";
const FETCH_TIMEOUT_MS = 8000;

export const REGISTRIES = {
  npm: "https://registry.npmjs.org/originlm",
  pypi: "https://pypi.org/pypi/originlm/json",
  crates: "https://crates.io/api/v1/crates/originlm",
};

const PRE_RANK = {
  dev: 0,
  a: 1,
  alpha: 1,
  b: 2,
  beta: 2,
  rc: 3,
  final: 99,
};

/**
 * Parse SemVer (e.g. "0.1.0-alpha.29", "1.2.3") or PEP 440 (e.g. "0.1.0a29", "0.1.0.dev1")
 * into a comparable tuple: [major, minor, patch, preRank, preNum].
 */
export function parseVersion(raw) {
  if (typeof raw !== "string") {
    throw new Error(`Invalid version: ${String(raw)}`);
  }
  const v = raw.trim().replace(/^v/i, "");
  const match = v.match(
    /^(\d+)\.(\d+)\.(\d+)(?:[-.]?(dev|alpha|beta|rc|a|b)\.?(\d+))?$/i,
  );
  if (!match) {
    throw new Error(`Unsupported version format: "${raw}"`);
  }
  const major = Number(match[1]);
  const minor = Number(match[2]);
  const patch = Number(match[3]);
  const preKind = match[4] ? match[4].toLowerCase() : "final";
  const preRank = PRE_RANK[preKind] ?? 0;
  const preNum = match[5] !== undefined ? Number(match[5]) : 0;
  return [major, minor, patch, preRank, preNum];
}

/** Return >0 if a > b, <0 if a < b, 0 if equal (treats 0.1.0-alpha.29 === 0.1.0a29). */
export function compareVersions(a, b) {
  const pa = parseVersion(a);
  const pb = parseVersion(b);
  for (let i = 0; i < pa.length; i++) {
    if (pa[i] !== pb[i]) return pa[i] - pb[i];
  }
  return 0;
}

function extractTomlSectionVersion(tomlText, sectionHeader) {
  const escaped = sectionHeader.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const sectionRe = new RegExp(`^\\[${escaped}\\]\\s*$`, "m");
  const start = tomlText.search(sectionRe);
  if (start === -1) return null;
  const afterHeader = tomlText.slice(start).replace(sectionRe, "");
  const nextSection = afterHeader.search(/^\[/m);
  const body = nextSection === -1 ? afterHeader : afterHeader.slice(0, nextSection);
  const verMatch = body.match(/^\s*version\s*=\s*["']([^"']+)["']/m);
  return verMatch ? verMatch[1].trim() : null;
}

export async function readRepoVersions(repoRoot = REPO_ROOT) {
  const [pkgRaw, cargoRaw, pyRaw] = await Promise.all([
    readFile(path.join(repoRoot, "package.json"), "utf8"),
    readFile(path.join(repoRoot, "Cargo.toml"), "utf8"),
    readFile(path.join(repoRoot, "packages", "sdk-python", "pyproject.toml"), "utf8"),
  ]);

  const npm = JSON.parse(pkgRaw)?.version ?? null;
  const crates = extractTomlSectionVersion(cargoRaw, "workspace.package");
  const pypi = extractTomlSectionVersion(pyRaw, "project");
  return { npm, pypi, crates };
}

async function fetchJson(url) {
  const res = await fetch(url, {
    headers: {
      Accept: "application/json",
      "User-Agent": USER_AGENT,
    },
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
  });
  if (!res.ok) {
    throw new Error(`HTTP ${res.status} from ${url}`);
  }
  return res.json();
}

export async function fetchRegistryVersions() {
  const fetchers = {
    npm: async () => {
      const data = await fetchJson(REGISTRIES.npm);
      const version = data?.["dist-tags"]?.alpha ?? data?.["dist-tags"]?.latest;
      if (!version) throw new Error(`Missing dist-tags.alpha in ${REGISTRIES.npm}`);
      return version;
    },
    pypi: async () => {
      const data = await fetchJson(REGISTRIES.pypi);
      const version = data?.info?.version;
      if (!version) throw new Error(`Missing info.version in ${REGISTRIES.pypi}`);
      return version;
    },
    crates: async () => {
      const data = await fetchJson(REGISTRIES.crates);
      const version = data?.crate?.max_version;
      if (!version) throw new Error(`Missing crate.max_version in ${REGISTRIES.crates}`);
      return version;
    },
  };

  const entries = await Promise.all(
    Object.entries(fetchers).map(async ([name, fn]) => {
      try {
        const version = await fn();
        return [name, { ok: true, version }];
      } catch (err) {
        return [name, { ok: false, error: err instanceof Error ? err.message : String(err) }];
      }
    }),
  );
  return Object.fromEntries(entries);
}

/**
 * Reconcile fetched registry versions with repo fallback versions and enforce the
 * unpublished-version guardrail.
 */
export function resolveVersions(fetched, repoVersions, warn = console.warn) {
  const resolved = {};
  const mismatches = [];
  const meta = {};

  for (const key of ["npm", "pypi", "crates"]) {
    const result = fetched[key];
    const repoVer = repoVersions[key];

    if (result?.ok) {
      const regVer = result.version;
      parseVersion(regVer);
      if (repoVer && compareVersions(repoVer, regVer) > 0) {
        mismatches.push(
          `${key}: repo version (${repoVer}) is ahead of published registry version (${regVer}) — publish is missing`,
        );
      }
      resolved[key] = regVer;
      meta[key] = "registry";
    } else {
      const reason = result?.error ?? "unknown error";
      if (!repoVer) {
        throw new Error(`Failed to fetch ${key} version (${reason}) and no repo fallback version was found.`);
      }
      parseVersion(repoVer);
      warn(`[fetch_versions] Warning: ${key} registry fetch failed (${reason}); falling back to repo version ${repoVer}`);
      resolved[key] = repoVer;
      meta[key] = "repo-fallback";
    }
  }

  if (mismatches.length > 0) {
    throw new Error(
      `Published registry guardrail failed — refusing to build with unpublished repo version(s):\n  - ${mismatches.join("\n  - ")}`,
    );
  }

  return { versions: resolved, sources: meta };
}

export function renderModule(versions) {
  return `// Generated at build time by scripts/fetch_versions.mjs — do not hand-edit.
export const PUBLISHED_VERSIONS = {
  npm: ${JSON.stringify(versions.npm)},
  pypi: ${JSON.stringify(versions.pypi)},
  crates: ${JSON.stringify(versions.crates)},
} as const;

export type PublishedVersions = typeof PUBLISHED_VERSIONS;
`;
}

async function main() {
  const [repoVersions, fetched] = await Promise.all([
    readRepoVersions(),
    fetchRegistryVersions(),
  ]);
  const { versions, sources } = resolveVersions(fetched, repoVersions);
  await writeFile(OUTPUT_FILE, renderModule(versions), "utf8");
  console.log(
    `[fetch_versions] npm=${versions.npm} (${sources.npm}), pypi=${versions.pypi} (${sources.pypi}), crates=${versions.crates} (${sources.crates})`,
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch((err) => {
    console.error(`[fetch_versions] ERROR: ${err instanceof Error ? err.message : String(err)}`);
    process.exit(1);
  });
}
