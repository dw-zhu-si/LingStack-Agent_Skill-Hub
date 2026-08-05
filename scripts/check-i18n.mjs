import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const source = readFileSync(join(root, "src/lib/i18n.ts"), "utf8");
const localeBlock = source.match(/export const localeCodes = \[([\s\S]*?)\] as const;/)?.[1] ?? "";
const locales = [...localeBlock.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
if (locales.length !== 11) throw new Error(`expected 11 locales, found ${locales.length}`);

const rowsBlock = source.match(/const rows: MessageRow\[\] = \[([\s\S]*?)\n\];/)?.[1] ?? "";
const rows = rowsBlock.split("\n").map((line) => line.trim()).filter((line) => line.startsWith("["))
  .map((line) => JSON.parse(line.replace(/,$/, "")));
const keys = new Set();
for (const row of rows) {
  if (row.length !== locales.length + 1) throw new Error(`${row[0]} has ${row.length - 1}/${locales.length} translations`);
  if (keys.has(row[0])) throw new Error(`duplicate translation key: ${row[0]}`);
  if (row.some((value) => typeof value !== "string" || value.trim() === "")) throw new Error(`empty translation in ${row[0]}`);
  keys.add(row[0]);
}

const componentDir = join(root, "src/lib/components");
const uiFiles = [join(root, "src/App.svelte"), ...readdirSync(componentDir).filter((name) => name.endsWith(".svelte")).map((name) => join(componentDir, name))];
const missing = new Set();
for (const file of uiFiles) {
  const content = readFileSync(file, "utf8");
  for (const match of content.matchAll(/\btr\(\s*"([^"]+)"/g)) if (!keys.has(match[1])) missing.add(match[1]);
}
if (missing.size) throw new Error(`missing translation keys: ${[...missing].sort().join(", ")}`);
console.log(`i18n coverage passed: ${locales.length} locales × ${rows.length} messages; ${keys.size} unique keys`);
