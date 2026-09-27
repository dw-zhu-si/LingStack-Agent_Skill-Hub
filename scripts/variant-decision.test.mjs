import assert from 'node:assert/strict';
import test from 'node:test';
import { evaluateVariantDecision } from '../src/lib/variantDecision.ts';

function setup(leftContent = 'Use safe defaults.', rightContent = 'Review changes.') {
  const asset = { logical_id: 'agent-test', variants: [
    { sha256: 'left-hash', locations: [{ path: '/left.md', source_class: 'Claude' }] },
    { sha256: 'right-hash', locations: [{ path: '/right.md', source_class: 'Codex' }] },
  ] };
  const doc = (sha256, content, path) => ({ logical_id: asset.logical_id, sha256, content, path });
  return { asset, record: undefined, left: doc('left-hash', leftContent, '/left.md'),
    right: doc('right-hash', rightContent, '/right.md'), targetSource: '', preference: 'stable' };
}
function record(input, selected = 'left-hash', status = 'verified_local') {
  return { logical_id: input.asset.logical_id, selected_sha256: selected, pending_refresh: false,
    verification: { sha256: selected, status } };
}

test('both valid, distinct registry documents are required before any recommendation', () => {
  for (const change of [i => i.left = null, i => i.right = null,
    i => i.left.logical_id = 'foreign', i => i.right.sha256 = 'unknown',
    i => i.right.sha256 = i.left.sha256]) {
    const input = setup(); input.targetSource = 'Codex'; change(input);
    const result = evaluateVariantDecision(input);
    assert.equal(result.recommendation, null); assert.equal(result.reason, 'unread');
    assert.equal(result.relation, 'unknown');
  }
});

test('unique registered target source outranks current selection and verification', () => {
  const input = setup(); input.record = record(input); input.targetSource = 'Codex';
  const result = evaluateVariantDecision(input);
  assert.equal(result.recommendation, 'right-hash'); assert.equal(result.reason, 'target');
  assert.equal(result.left.sourceMatch, false); assert.equal(result.right.sourceMatch, true);
});

test('stable prefers current; evidence prefers valid same-hash verification', () => {
  const input = setup(); input.record = record(input);
  assert.equal(evaluateVariantDecision(input).reason, 'current');
  input.preference = 'evidence';
  assert.equal(evaluateVariantDecision(input).reason, 'verified');
  assert.equal(evaluateVariantDecision(input).left.verified, true);
  input.record = undefined;
  assert.equal(evaluateVariantDecision(input).recommendation, null);
});

test('pending, nonverified, unselected and foreign records never count as valid evidence', () => {
  for (const change of [r => r.pending_refresh = true, r => r.verification.status = 'failed',
    r => r.verification.sha256 = 'right-hash', r => r.selected_sha256 = 'unknown',
    r => r.logical_id = 'foreign']) {
    const input = setup(); input.record = record(input); input.preference = 'evidence'; change(input.record);
    const result = evaluateVariantDecision(input);
    assert.equal(result.left.verified, false); assert.equal(result.right.verified, false);
    assert.notEqual(result.reason, 'verified');
  }
});

test('full equality and cross-format core equality are distinct relations', () => {
  const same = setup('same', 'same');
  assert.equal(evaluateVariantDecision(same).relation, 'identical');
  const input = setup('---\nname: test\n---\nDo work.\n', 'developer_instructions = "Do work."');
  input.right.path = '/right.toml';
  const result = evaluateVariantDecision(input);
  assert.equal(result.relation, 'core_same'); assert.equal(result.reason, 'tie');
});

test('only blank lines and plain Markdown separators are ignored', () => {
  const input = setup('First.\n\n---\nSecond.', 'First.\nSecond.');
  assert.equal(evaluateVariantDecision(input).relation, 'format_only');
  for (const left of ['# First.\nSecond.', 'First.\n- Second.', 'First!\nSecond.',
    'First.\n    ---\nSecond.', 'First.\n---\nSecond.', 'First.\n```\n---\n```\nSecond.']) {
    assert.equal(evaluateVariantDecision(setup(left, 'First.\nSecond.')).relation, 'changed');
  }
});

test('core content changes require review when no unique selection evidence exists', () => {
  const result = evaluateVariantDecision(setup('Allow writes.', 'Deny writes.'));
  assert.equal(result.relation, 'changed'); assert.equal(result.reason, 'review');
  assert.equal(result.recommendation, null); assert.equal(result.added, 1); assert.equal(result.removed, 1);
});

test('unsupported format does not get an optimistic core-equivalence classification', () => {
  const input = setup('a\n\nb', 'a\nb'); input.left.path = '/left.json'; input.right.path = '/right.json';
  const result = evaluateVariantDecision(input);
  assert.equal(result.relation, 'unknown');
  assert.equal(result.reason, 'review');
  assert.ok(result.added + result.removed > 0);
});

test('truncated matching prefixes never establish no content changes', () => {
  const prefix = 'Same line.\n'.repeat(800);
  const result = evaluateVariantDecision(setup(prefix + 'Old.', prefix + 'New.'));
  assert.equal(result.relation, 'changed'); assert.equal(result.reason, 'review');
  assert.equal(result.truncated, true); assert.equal(result.added + result.removed, 0);
  const identical = evaluateVariantDecision(setup(prefix, prefix));
  assert.equal(identical.relation, 'identical'); assert.equal(identical.truncated, false);
});


test('target source on a third version cannot fall back to either compared version', () => {
  const input = setup(); input.record = record(input); input.targetSource = 'Cursor';
  input.asset.variants.push({ sha256: 'third-hash', locations: [{ source_class: 'Cursor' }] });
  for (const preference of ['stable', 'evidence']) {
    input.preference = preference;
    const result = evaluateVariantDecision(input);
    assert.equal(result.recommendation, null);
    assert.equal(result.reason, 'no_target');
  }
});

test('evidence preference refuses unverified current selection', () => {
  const input = setup(); input.record = record(input); delete input.record.verification;
  assert.equal(evaluateVariantDecision(input).recommendation, 'left-hash');
  input.preference = 'evidence';
  const result = evaluateVariantDecision(input);
  assert.equal(result.recommendation, null);
  assert.equal(result.reason, 'review');
  input.right.content = input.left.content;
  assert.equal(evaluateVariantDecision(input).reason, 'tie');
  input.record = record(input);
  const verified = evaluateVariantDecision(input);
  assert.equal(verified.recommendation, 'left-hash');
  assert.equal(verified.reason, 'verified');
});
