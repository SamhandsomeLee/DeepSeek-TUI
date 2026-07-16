const assert = require("node:assert/strict");
const test = require("node:test");

const pkg = require("../package.json");
const {
  assertChecksumManifestIncludes,
  assertPackageVersionMatchesBinaryVersion,
  assertReleaseAssetsFresh,
  parseChecksumManifest,
} = require("../scripts/verify-release-assets");

test("parseChecksumManifest accepts GNU and BSD filename forms", () => {
  const manifest = parseChecksumManifest(
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  codewhale-linux-x64",
      "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb *codewhale-windows-x64.exe",
    ].join("\n"),
  );

  assert.equal(manifest.get("codewhale-linux-x64"), "a".repeat(64));
  assert.equal(manifest.get("codewhale-windows-x64.exe"), "b".repeat(64));
});

test("parseChecksumManifest rejects malformed checksum rows", () => {
  assert.throws(
    () => parseChecksumManifest("not-a-sha  codewhale-linux-x64"),
    /Invalid checksum manifest line/,
  );
});

test("assertReleaseAssetsFresh rejects missing release assets", () => {
  assert.throws(
    () =>
      assertReleaseAssetsFresh(
        { assets: [{ name: "codewhale-linux-x64", state: "uploaded", updated_at: "2026-06-26T00:10:00Z" }] },
        ["codewhale-linux-x64", "codewhale-artifacts-sha256.txt"],
        { database_id: 123, created_at: "2026-06-26T00:00:00Z" },
      ),
    /missing required release asset/,
  );
});

test("assertChecksumManifestIncludes rejects missing bundle manifest and archive rows", () => {
  const manifest = parseChecksumManifest(
    `${"a".repeat(64)}  codewhale-linux-x64.tar.gz`,
  );

  assert.throws(
    () =>
      assertChecksumManifestIncludes(
        manifest,
        ["codewhale-linux-x64.tar.gz", "codewhale-bundles-sha256.txt"],
        "Canonical checksum manifest",
      ),
    /Canonical checksum manifest is missing codewhale-bundles-sha256\.txt/,
  );
});

test("bundle checksum rows use public archive basenames", () => {
  const manifest = parseChecksumManifest(
    `${"a".repeat(64)}  bundles/codewhale-linux-x64.tar.gz`,
  );

  assert.throws(
    () =>
      assertChecksumManifestIncludes(
        manifest,
        ["codewhale-linux-x64.tar.gz"],
        "Bundle checksum manifest",
      ),
    /Bundle checksum manifest is missing codewhale-linux-x64\.tar\.gz/,
  );
});

test("assertReleaseAssetsFresh rejects assets older than the release workflow run", () => {
  assert.throws(
    () =>
      assertReleaseAssetsFresh(
        { assets: [{ name: "codewhale-linux-x64", state: "uploaded", updated_at: "2026-06-25T23:59:59Z" }] },
        ["codewhale-linux-x64"],
        { database_id: 123, created_at: "2026-06-26T00:00:00Z" },
      ),
    /asset set is stale/,
  );
});

test("assertReleaseAssetsFresh rejects non-uploaded assets", () => {
  assert.throws(
    () =>
      assertReleaseAssetsFresh(
        { assets: [{ name: "codewhale-linux-x64", state: "new", updated_at: "2026-06-26T00:10:00Z" }] },
        ["codewhale-linux-x64"],
        { database_id: 123, created_at: "2026-06-26T00:00:00Z" },
      ),
    /asset set is stale/,
  );
});

test("assertReleaseAssetsFresh accepts assets updated by the release workflow run", () => {
  assert.doesNotThrow(() =>
    assertReleaseAssetsFresh(
      { assets: [{ name: "codewhale-linux-x64", state: "uploaded", updated_at: "2026-06-26T00:10:00Z" }] },
      ["codewhale-linux-x64"],
      { database_id: 123, created_at: "2026-06-26T00:00:00Z" },
    ),
  );
});

test("assertPackageVersionMatchesBinaryVersion allows packaging-only releases only with an explicit override", () => {
  assert.doesNotThrow(() => assertPackageVersionMatchesBinaryVersion(pkg.version));
  assert.throws(
    () => assertPackageVersionMatchesBinaryVersion("0.0.0-packaging-test"),
    /does not match codewhaleBinaryVersion/,
  );

  const previous = process.env.CODEWHALE_ALLOW_NPM_BINARY_MISMATCH;
  process.env.CODEWHALE_ALLOW_NPM_BINARY_MISMATCH = "1";
  try {
    assert.doesNotThrow(() => assertPackageVersionMatchesBinaryVersion("0.0.0-packaging-test"));
  } finally {
    if (previous === undefined) {
      delete process.env.CODEWHALE_ALLOW_NPM_BINARY_MISMATCH;
    } else {
      process.env.CODEWHALE_ALLOW_NPM_BINARY_MISMATCH = previous;
    }
  }
});

test("npm publication is guarded by an exact clean release-tag checkout", () => {
  assert.match(
    pkg.scripts.prepublishOnly,
    /require-release-tag-checkout\.sh && node scripts\/verify-release-assets\.js/,
  );
});
