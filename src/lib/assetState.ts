import type { AssetGovernanceRecord, AssetGroup } from "./types";

/** A receipt is valid only for the currently selected, still-present content. */
export function hasValidVerification(asset: AssetGroup, record?: AssetGovernanceRecord): boolean {
  const receipt = record?.verification;
  return record?.logical_id === asset.logical_id && !!receipt && receipt.status === "verified_local" && !record?.pending_refresh
    && receipt.sha256 === record?.selected_sha256
    && asset.variants.some(variant => variant.sha256 === receipt.sha256);
}

/** Preserve form edits for receipt updates; invalidate when content versions change. */
export function assetVersionKey(asset: AssetGroup): string {
  return JSON.stringify([asset.logical_id, [...new Set(asset.variants.map(variant => variant.sha256))].sort()]);
}
