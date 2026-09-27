/** Bounded, local-only line comparison. Counts always describe the returned rows. */
export type ComparisonRow = {
  kind: 'equal' | 'added' | 'removed';
  /** Original line, including its line terminator when present. */
  text: string;
  leftLine: number | null;
  rightLine: number | null;
};

export type DefinitionComparison = {
  rows: ComparisonRow[];
  added: number;
  removed: number;
  truncated: boolean;
  /** When truncated, counts cover only the compared prefixes, never the full documents. */
  countsScope: 'complete' | 'compared-prefix';
  budgetReason: string | null;
};

// At most 491,401 DP cells (under 1 MiB) and 256 Ki UTF-16 code units total.
const MAX_LINES = 700;
const MAX_CHARS = 128 * 1024;

type BoundedLines = { lines: string[]; truncated: boolean };

function boundedLines(content: string): BoundedLines {
  const lines: string[] = [];
  const end = Math.min(content.length, MAX_CHARS);
  let start = 0;
  // Scan only the bounded prefix. Keep terminators: CRLF, LF and EOF are real differences.
  for (let index = 0; index < end; index += 1) {
    if (content.charCodeAt(index) !== 10) continue;
    lines.push(content.slice(start, index + 1));
    start = index + 1;
    if (lines.length === MAX_LINES) {
      return { lines, truncated: start < content.length };
    }
  }
  if (end < content.length) {
    // Never present a cut-off line as if it were a complete source line.
    return { lines, truncated: true };
  }
  if (start < content.length) lines.push(content.slice(start));
  return { lines, truncated: false };
}

/**
 * Exact minimal insert/delete diff within a bounded prefix of each document.
 * Row numbers are one-based source line numbers; missing sides are null.
 * No content is executed, interpreted as instructions or used to rank versions.
 */
export function compareDefinitions(left: string, right: string): DefinitionComparison {
  const a = boundedLines(left);
  const b = boundedLines(right);
  const width = b.lines.length + 1;
  const lcs = new Uint16Array((a.lines.length + 1) * width);
  // Intern strings once, avoiding repeated long-string comparisons inside the quadratic loop.
  const ids = new Map<string, number>();
  const intern = (line: string): number => {
    const existing = ids.get(line);
    if (existing !== undefined) return existing;
    const next = ids.size;
    ids.set(line, next);
    return next;
  };
  const aIds = a.lines.map(intern);
  const bIds = b.lines.map(intern);
  for (let i = a.lines.length - 1; i >= 0; i -= 1) {
    for (let j = b.lines.length - 1; j >= 0; j -= 1) {
      lcs[i * width + j] = aIds[i] === bIds[j]
        ? lcs[(i + 1) * width + j + 1] + 1
        : Math.max(lcs[(i + 1) * width + j], lcs[i * width + j + 1]);
    }
  }
  const rows: ComparisonRow[] = [];
  let i = 0;
  let j = 0;
  let added = 0;
  let removed = 0;
  while (i < a.lines.length || j < b.lines.length) {
    if (i < a.lines.length && j < b.lines.length && aIds[i] === bIds[j]) {
      rows.push({ kind: 'equal', text: a.lines[i], leftLine: i + 1, rightLine: j + 1 });
      i += 1;
      j += 1;
    } else if (i < a.lines.length && (j === b.lines.length
      || lcs[(i + 1) * width + j] >= lcs[i * width + j + 1])) {
      rows.push({ kind: 'removed', text: a.lines[i], leftLine: i + 1, rightLine: null });
      i += 1;
      removed += 1;
    } else {
      rows.push({ kind: 'added', text: b.lines[j], leftLine: null, rightLine: j + 1 });
      j += 1;
      added += 1;
    }
  }
  const truncated = a.truncated || b.truncated;
  return {
    rows, added, removed, truncated,
    countsScope: truncated ? 'compared-prefix' : 'complete',
    budgetReason: truncated
      ? '每侧最多比较前 700 行、128 Ki UTF-16 字符；未纳入截断行及后续内容，新增和删除数量仅针对已比较部分。'
      : null,
  };
}

export type ComparableDefinition = {
  text: string;
  extracted: boolean;
  format: 'markdown' | 'toml' | 'raw';
};

/**
 * Extract a narrowly supported instruction payload, never evaluate a definition.
 * Equality of extracted text is not file equivalence: metadata and wrappers may differ.
 * Unsupported or ambiguous input is returned byte-for-byte as the original JS string.
 */
export function extractComparableDefinition(content: string, path: string): ComparableDefinition {
  const lowerPath = path.toLowerCase();
  const format: ComparableDefinition['format'] = lowerPath.endsWith('.md')
    ? 'markdown' : lowerPath.endsWith('.toml') ? 'toml' : 'raw';
  const fallback = (): ComparableDefinition => ({ text: content, extracted: false, format });
  const normalize = (text: string): string => text.replace(/\r\n/g, '\n').trim();
  // Extraction has a separate linear bound; oversized definitions remain available in raw diff.
  if (content.length > 2 * 1024 * 1024) return fallback();
  if (format === 'markdown') {
    const normalized = content.replace(/\r\n/g, '\n');
    let body = normalized;
    if (/^---[ \t]*\n/.test(normalized)) {
      const openingEnd = normalized.indexOf('\n') + 1;
      const rest = normalized.slice(openingEnd);
      const closing = /^---[ \t]*(?:\n|$)/m.exec(rest);
      if (!closing) return fallback();
      body = rest.slice(closing.index + closing[0].length);
    }
    return { text: normalize(body), extracted: true, format };
  }
  if (format !== 'toml') return fallback();

  let inSection = false;
  let payload: string | undefined;
  for (const line of content.replace(/\r\n/g, '\n').split('\n')) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;
    // Multiline TOML strings require a real parser; reject rather than guess their boundaries.
    if (trimmed.includes('"""') || trimmed.includes("'''")) return fallback();
    if (trimmed.startsWith('[')) {
      inSection = true;
      continue;
    }
    // Quoted and dotted forms are deliberately unsupported, including nested instruction keys.
    if (/^(?:[\w-]+\.)*(?:developer_instructions|"developer_instructions"|'developer_instructions')\s*=/.test(trimmed)) {
      if (inSection || payload !== undefined || !/^developer_instructions\s*=/.test(trimmed)) return fallback();
      const value = trimmed.slice(trimmed.indexOf('=') + 1).trim();
      if (!value.startsWith('"') || !value.endsWith('"')) return fallback();
      try {
        const decoded: unknown = JSON.parse(value);
        if (typeof decoded !== 'string') return fallback();
        payload = decoded;
      } catch {
        return fallback();
      }
    } else {
      // Only simple scalar siblings are understood. Reject arrays/tables/continuations so an
      // apparent assignment inside an unfinished composite value is never mistaken for a root key.
      const sibling = /^[A-Za-z_][\w-]*\s*=\s*(.+)$/.exec(trimmed);
      if (!sibling) return fallback();
      try {
        const value: unknown = JSON.parse(sibling[1]);
        if (value === null || !['string', 'number', 'boolean'].includes(typeof value)) return fallback();
      } catch {
        return fallback();
      }
    }
  }
  return payload === undefined ? fallback() : { text: normalize(payload), extracted: true, format };
}
