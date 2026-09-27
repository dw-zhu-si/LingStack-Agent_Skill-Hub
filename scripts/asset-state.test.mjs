import assert from "node:assert/strict";
import test from "node:test";
import { assetVersionKey, hasValidVerification } from "../src/lib/assetState.ts";

const asset = { logical_id: "skill:test", variants: [{ sha256: "a" }, { sha256: "b" }] };
const receipt = { status: "verified_local", sha256: "a" };
const record = { logical_id: asset.logical_id, selected_sha256: "a", pending_refresh: false, verification: receipt };

test("verification follows selected live content and accepted receipt status", () => {
  assert.equal(hasValidVerification(asset, record), true);
  for (const invalid of [undefined, {}, { ...record, logical_id: "skill:other" }, { ...record, pending_refresh: true },
    { ...record, selected_sha256: "b" }, { ...record, selected_sha256: "" },
    { ...record, verification: { ...receipt, status: "unverified" } },
    { ...record, selected_sha256: "missing", verification: { ...receipt, sha256: "missing" } }]) {
    assert.equal(hasValidVerification(asset, invalid), false);
  }
  assert.equal(hasValidVerification({ ...asset, variants: [{ sha256: "b" }] }, record), false);
});

test("form identity survives receipt, metadata, order and duplicate changes but not content replacement", () => {
  const original = assetVersionKey(asset);
  assert.equal(original, assetVersionKey({ ...asset, name: "renamed", variants: [{ sha256: "b" }, { sha256: "a" }, { sha256: "a" }] }));
  assert.notEqual(original, assetVersionKey({ ...asset, variants: [{ sha256: "c" }] }));
  assert.notEqual(original, assetVersionKey({ ...asset, logical_id: "skill:other" }));
});
