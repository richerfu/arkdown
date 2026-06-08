#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

function usage() {
  console.error(
    'usage: audit-require-graph.js --root <dir> [--root <dir>...] --entry <file> [--entry <file>...]'
  );
  process.exit(2);
}

const roots = [];
const entries = [];
for (let i = 2; i < process.argv.length; i += 1) {
  const arg = process.argv[i];
  if (arg === '--root') {
    const value = process.argv[++i];
    if (!value) usage();
    roots.push(path.resolve(value));
  } else if (arg === '--entry') {
    const value = process.argv[++i];
    if (!value) usage();
    entries.push(value);
  } else {
    usage();
  }
}

if (!roots.length || !entries.length) usage();

function existsFile(file) {
  return fs.existsSync(file) && fs.statSync(file).isFile();
}

function inRoots(file) {
  return roots.some((root) => file.startsWith(root + path.sep) || file === root);
}

function resolveEntry(entry) {
  if (path.isAbsolute(entry)) return entry;
  for (const root of roots) {
    const candidate = path.resolve(root, entry);
    if (existsFile(candidate)) return candidate;
  }
  throw new Error(`entry not found under roots: ${entry}`);
}

function resolveLocal(from, specifier) {
  if (!specifier.startsWith('.')) return null;
  const base = path.resolve(path.dirname(from), specifier);
  const candidates = [base, `${base}.js`, path.join(base, 'index.js')];
  return candidates.find(existsFile) || null;
}

function relativeToRoots(file) {
  const root = roots.find((candidate) => file.startsWith(candidate + path.sep) || file === candidate);
  return root ? path.relative(root, file) : file;
}

const seen = new Set();
function scan(file) {
  if (seen.has(file)) return;
  seen.add(file);
  const source = fs.readFileSync(file, 'utf8');
  const requirePattern = /require\(["']([^"']+)["']\)/g;
  let match;
  while ((match = requirePattern.exec(source))) {
    const next = resolveLocal(file, match[1]);
    if (next && inRoots(next)) scan(next);
  }
}

for (const entry of entries.map(resolveEntry)) scan(entry);

const files = [...seen].sort().map(relativeToRoots);
console.log(`count=${files.length}`);
for (const file of files) console.log(file);
