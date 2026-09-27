import assert from 'node:assert/strict';
import test from 'node:test';
import { compareDefinitions } from '../src/lib/variantComparison.ts';

function reconstruct(result, side) {
  return result.rows.filter(row => row[side === 'left' ? 'leftLine' : 'rightLine'] !== null)
    .map(row => row.text).join('');
}

test('empty, insertion, deletion and one-based source lines', () => {
  assert.deepEqual(compareDefinitions('', '').rows, []);
  const inserted = compareDefinitions('', '中文\n第二行');
  assert.equal(inserted.added, 2);
  assert.equal(inserted.removed, 0);
  assert.deepEqual(inserted.rows.map(row => [row.leftLine, row.rightLine]), [[null, 1], [null, 2]]);
  const removed = compareDefinitions('中文\n第二行', '');
  assert.equal(removed.removed, 2);
  assert.equal(removed.added, 0);
  assert.equal(reconstruct(removed, 'left'), '中文\n第二行');
});

test('repeated lines yield minimal deterministic edit counts and reconstruct both sides', () => {
  const left = '同\n同\n旧\n末\n';
  const right = '同\n新\n同\n末\n';
  const result = compareDefinitions(left, right);
  assert.equal(result.added, 1);
  assert.equal(result.removed, 1);
  assert.equal(result.rows.filter(row => row.kind === 'equal').length, 3);
  assert.equal(reconstruct(result, 'left'), left);
  assert.equal(reconstruct(result, 'right'), right);
  assert.deepEqual(compareDefinitions(left, right), result);
});

test('CRLF and trailing newline differences are preserved, not called equal', () => {
  for (const [left, right] of [['a\r\n', 'a\n'], ['a', 'a\n'], ['', '\n'], ['\r', '']]) {
    const result = compareDefinitions(left, right);
    assert.ok(result.added + result.removed > 0);
    assert.equal(reconstruct(result, 'left'), left);
    assert.equal(reconstruct(result, 'right'), right);
  }
  assert.equal(compareDefinitions('a\r\n', 'a\r\n').added, 0);
});

test('metadata-like text remains exact literal text without semantic equivalence claims', () => {
  const left = '---\nversion: 1\n---\n正文\n';
  const right = '---\nversion: 2\n---\n正文\n';
  const result = compareDefinitions(left, right);
  assert.equal(result.added, 1);
  assert.equal(result.removed, 1);
  assert.equal(result.rows.find(row => row.kind === 'removed').leftLine, 2);
});

test('line and character budgets are explicit; counts describe returned prefixes only', () => {
  const result = compareDefinitions('旧\n'.repeat(20000), '新\n'.repeat(20000));
  assert.equal(result.truncated, true);
  assert.equal(result.countsScope, 'compared-prefix');
  assert.ok(result.budgetReason);
  assert.equal(result.added, 700);
  assert.equal(result.removed, 700);
  assert.equal(result.rows.length, 1400);
  const longLine = compareDefinitions('x'.repeat(2_000_000), 'y'.repeat(2_000_000));
  assert.equal(longLine.truncated, true);
  assert.equal(longLine.rows.length, 0);
  assert.equal(longLine.added, 0);
  assert.equal(compareDefinitions('a\n'.repeat(700), 'a\n'.repeat(700)).truncated, false);
});

test('source numbers increase monotonically and edits reconstruct diverse short inputs', () => {
  const samples = ['', 'a', 'a\n', 'b\na\n', 'a\na\nb', '中\r\n文', '\n\n'];
  for (const left of samples) for (const right of samples) {
    const result = compareDefinitions(left, right);
    assert.equal(reconstruct(result, 'left'), left);
    assert.equal(reconstruct(result, 'right'), right);
    for (const key of ['leftLine', 'rightLine']) {
      const numbers = result.rows.map(row => row[key]).filter(number => number !== null);
      assert.deepEqual(numbers, numbers.map((_, i) => i + 1));
    }
    assert.equal(result.added, result.rows.filter(row => row.kind === 'added').length);
    assert.equal(result.removed, result.rows.filter(row => row.kind === 'removed').length);
  }
});

const { extractComparableDefinition } = await import('../src/lib/variantComparison.ts');

test('instruction extraction compares Markdown and JSON-compatible TOML wrappers', () => {
  const core = '# API 测试员\n\n检查接口。';
  const md = extractComparableDefinition(`---\r\nname: API Tester\r\n---\r\n\r\n${core}\r\n`, 'SKILL.md');
  const toml = extractComparableDefinition(`name = "API Tester"\ndeveloper_instructions = ${JSON.stringify(` \r\n${core}\r\n `)}\n`, 'agent.toml');
  assert.equal(md.extracted, true);
  assert.equal(toml.extracted, true);
  assert.equal(md.format, 'markdown');
  assert.equal(toml.format, 'toml');
  assert.equal(md.text, core);
  assert.equal(toml.text, core);
  assert.equal(compareDefinitions(md.text, toml.text).added, 0);
  const changed = extractComparableDefinition(`developer_instructions = ${JSON.stringify(`${core}\n新增检查`)}`, 'agent.toml');
  assert.notEqual(changed.text, md.text);
});

test('Markdown only removes a complete opening YAML block', () => {
  const unclosed = '---\r\nname: x\r\n正文';
  assert.deepEqual(extractComparableDefinition(unclosed, 'a.md'), { text: unclosed, extracted: false, format: 'markdown' });
  assert.equal(extractComparableDefinition('正文\n---\nname: x\n---\n', 'a.md').text, '正文\n---\nname: x\n---');
  assert.equal(extractComparableDefinition('---\nname: x\n---', 'a.md').text, '');
  assert.equal(extractComparableDefinition(' 正文\r\n', 'a.md').text, '正文');
});

test('invalid, duplicate, nested, multiline and unsupported TOML remain unchanged', () => {
  const invalid = [
    'developer_instructions = "bad\\q"',
    'developer_instructions = "a"\ndeveloper_instructions = "b"',
    '[agent]\ndeveloper_instructions = "a"',
    'developer_instructions = "a"\n[agent]\ndeveloper_instructions = "b"',
    'agent.developer_instructions = "a"',
    'tools = [\ndeveloper_instructions = "a"',
    'metadata = {\ndeveloper_instructions = "a"',
    '"developer_instructions" = "a"',
    'developer_instructions = """multi\nline"""',
    "developer_instructions = '''multi\nline'''",
    'developer_instructions = "a\nb"',
    'developer_instructions = { text = "a" }',
    'developer_instructions = ["a"]',
    'developer_instructions = "a" # unsupported trailing comment',
    'description = "only metadata"',
  ];
  for (const content of invalid) {
    assert.deepEqual(extractComparableDefinition(content, 'a.toml'), { text: content, extracted: false, format: 'toml' });
  }
  const raw = ' developer_instructions = "a"\r\n';
  assert.deepEqual(extractComparableDefinition(raw, 'a.json'), { text: raw, extracted: false, format: 'raw' });
  assert.equal(extractComparableDefinition('# developer_instructions = "ignored"\ndeveloper_instructions = ""', 'a.toml').extracted, true);
});
