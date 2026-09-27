import type { AssetDefinition, AssetGovernanceRecord, AssetGroup } from './types';
import { compareDefinitions, extractComparableDefinition } from './variantComparison.ts';
import { hasValidVerification } from './assetState.ts';

export type VariantDecisionInput = {
  asset: AssetGroup;
  record: AssetGovernanceRecord | undefined;
  left: AssetDefinition | null;
  right: AssetDefinition | null;
  targetSource: string;
  preference: 'stable' | 'evidence';
};
export type VariantDecisionSide = { sourceMatch: boolean; verified: boolean; current: boolean };
export type VariantDecision = {
  recommendation: string | null;
  reason: 'unread' | 'no_target' | 'target' | 'verified' | 'current' | 'tie' | 'review';
  relation: 'identical' | 'core_same' | 'format_only' | 'changed' | 'unknown';
  left: VariantDecisionSide;
  right: VariantDecisionSide;
  added: number;
  removed: number;
  truncated: boolean;
};

/** Remove only blank lines and standalone Markdown thematic breaks outside fenced code.
 * Never strip headings, list markers, inline punctuation, or content inside code fences.
 */
function withoutFormattingLines(text: string): string {
  let fence: { marker: string; length: number } | null = null;
  return text.split('\n').filter((line, index, lines) => {
    const opening = /^ {0,3}(`{3,}|~{3,})(.*)$/.exec(line);
    if (fence) {
      if (opening && opening[1][0] === fence.marker && opening[1].length >= fence.length
        && opening[2].trim() === '') fence = null;
      return true;
    }
    if (opening) {
      fence = { marker: opening[1][0], length: opening[1].length };
      return true;
    }
    if (!line.trim()) return false;
    // A dash underline directly below text may be a Setext heading, not decoration.
    if (/^ {0,3}-{3,}[ \t]*$/.test(line) && index > 0 && lines[index - 1].trim()) return true;
    // Four-space indentation denotes code; it is not a thematic break.
    return !/^ {0,3}(?:(?:\*[ \t]*){3,}|(?:-[ \t]*){3,}|(?:_[ \t]*){3,})$/.test(line);
  }).join('\n');
}

/** Pure decision support using definitions already hash-checked by the desktop read API.
 * Matching a registered source is not proof of compatibility. No file changes or model calls.
 */
export function evaluateVariantDecision(input: VariantDecisionInput): VariantDecision {
  const { asset, record, left, right, targetSource, preference } = input;
  const validDocument = (doc: AssetDefinition | null): doc is AssetDefinition => !!doc
    && doc.logical_id === asset.logical_id && typeof doc.content === 'string'
    && !!doc.sha256 && asset.variants.some(variant => variant.sha256 === doc.sha256);
  const validRecord = record?.logical_id === asset.logical_id ? record : undefined;
  function sideState(doc: AssetDefinition | null): VariantDecisionSide {
    if (!validDocument(doc)) return { sourceMatch: false, verified: false, current: false };
    const variant = asset.variants.find(item => item.sha256 === doc.sha256)!;
    const current = validRecord?.selected_sha256 === doc.sha256;
    return {
      sourceMatch: !targetSource || variant.locations.some(location => location.source_class === targetSource),
      current,
      verified: current && hasValidVerification(asset, validRecord),
    };
  }
  const decision: VariantDecision = {
    recommendation: null, reason: 'unread', relation: 'unknown',
    left: sideState(left), right: sideState(right), added: 0, removed: 0, truncated: false,
  };
  if (!validDocument(left) || !validDocument(right) || left.sha256 === right.sha256) return decision;

  const a = extractComparableDefinition(left.content, left.path);
  const b = extractComparableDefinition(right.content, right.path);
  const extracted = a.extracted && b.extracted;
  const leftText = extracted ? a.text : left.content;
  const rightText = extracted ? b.text : right.content;
  if (left.content === right.content) decision.relation = 'identical';
  else if (extracted && leftText === rightText) decision.relation = 'core_same';
  else if (extracted && withoutFormattingLines(leftText) === withoutFormattingLines(rightText)) {
    decision.relation = 'format_only';
  } else {
    decision.relation = extracted ? 'changed' : 'unknown';
    const diff = compareDefinitions(leftText, rightText);
    decision.added = diff.added;
    decision.removed = diff.removed;
    decision.truncated = diff.truncated;
  }

  function chooseUnique(field: keyof VariantDecisionSide, reason: VariantDecision['reason']): boolean {
    if (decision.left[field] === decision.right[field]) return false;
    decision.recommendation = decision.left[field] ? left!.sha256 : right!.sha256;
    decision.reason = reason;
    return true;
  }
  if (targetSource && !decision.left.sourceMatch && !decision.right.sourceMatch) {
    decision.reason = 'no_target';
    return decision;
  }
  if (targetSource && chooseUnique('sourceMatch', 'target')) return decision;
  const order = preference === 'stable' ? ['current', 'verified'] as const : ['verified'] as const;
  for (const criterion of order) if (chooseUnique(criterion, criterion)) return decision;
  decision.reason = decision.relation === 'changed' || decision.relation === 'unknown' ? 'review' : 'tie';
  return decision;
}
