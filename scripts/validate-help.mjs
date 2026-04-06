#!/usr/bin/env node

/**
 * validate-help.mjs
 * Validates help article markdown files in docs/help/.
 * Checks for required YAML frontmatter fields, broken internal slug references,
 * and referenced images that don't exist.
 */

import { readdir, readFile, access } from "node:fs/promises";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const HELP_DIR = resolve(__dirname, "..", "docs", "help");

const REQUIRED_FIELDS = ["title", "slug", "category", "order", "schema_version"];

let errorCount = 0;
let warnCount = 0;

function error(file, message) {
  errorCount++;
  console.error(`  ❌ [${file}] ${message}`);
}

function warn(file, message) {
  warnCount++;
  console.warn(`  ⚠️  [${file}] ${message}`);
}

function parseFrontmatter(content) {
  const match = /^---\r?\n([\s\S]*?)\r?\n---/.exec(content);
  if (!match) return null;

  const fields = {};
  for (const line of match[1].split("\n")) {
    const colonIdx = line.indexOf(":");
    if (colonIdx === -1) continue;
    const key = line.slice(0, colonIdx).trim();
    let value = line.slice(colonIdx + 1).trim();
    // Strip surrounding quotes
    if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
      value = value.slice(1, -1);
    }
    fields[key] = value;
  }
  return fields;
}

async function fileExists(filePath) {
  try {
    await access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function collectArticles(mdFiles, slugMap, articleContents) {
  for (const file of mdFiles) {
    const filePath = join(HELP_DIR, file);
    const content = await readFile(filePath, "utf-8");
    articleContents.set(file, content);

    const frontmatter = parseFrontmatter(content);
    if (!frontmatter) {
      error(file, "Missing YAML frontmatter (must start with ---)");
      continue;
    }

    for (const field of REQUIRED_FIELDS) {
      if (!frontmatter[field]) {
        error(file, `Missing required frontmatter field: "${field}"`);
      }
    }

    if (frontmatter.slug) {
      if (slugMap.has(frontmatter.slug)) {
        error(file, `Duplicate slug "${frontmatter.slug}" (also in ${slugMap.get(frontmatter.slug)})`);
      } else {
        slugMap.set(frontmatter.slug, file);
      }
    }
  }
}

function isExternalUrl(target) {
  return target.startsWith("http://") || target.startsWith("https://") || target.startsWith("mailto:");
}

async function validateLink(file, target, slugMap) {
  if (isExternalUrl(target)) {
    return;
  }

  if (/\.(png|jpg|jpeg|gif|svg|webp)$/i.test(target)) {
    const imgPath = resolve(HELP_DIR, target);
    const exists = await fileExists(imgPath);
    if (!exists) {
      error(file, `Referenced image not found: "${target}"`);
    }
    return;
  }

  if (!target.includes("/") && !target.includes(".") && !target.startsWith("#")) {
    if (!slugMap.has(target)) {
      warn(file, `Internal link to unknown slug: "${target}"`);
    }
  }
}

async function validateLinks(articleContents, slugMap) {
  for (const [file, content] of articleContents) {
    const linkRegex = /\[([^\]]*)\]\(([^)]+)\)/g;
    let match;
    while ((match = linkRegex.exec(content)) !== null) {
      await validateLink(file, match[2], slugMap);
    }
  }
}

function printSummary(fileCount) {
  console.log(`\nScanned ${fileCount} files.`);
  console.log(`  Errors: ${errorCount}`);
  console.log(`  Warnings: ${warnCount}`);

  if (errorCount > 0) {
    process.exit(1);
  }

  console.log("\n✅ All help articles are valid.");
}

async function main() {
  console.log("Validating help articles in docs/help/\n");

  let files;
  try {
    files = await readdir(HELP_DIR);
  } catch {
    console.error(`Could not read directory: ${HELP_DIR}`);
    process.exit(1);
  }

  const mdFiles = files.filter((f) => f.endsWith(".md"));

  if (mdFiles.length === 0) {
    console.error("No .md files found in docs/help/");
    process.exit(1);
  }

  const slugMap = new Map();
  const articleContents = new Map();

  await collectArticles(mdFiles, slugMap, articleContents);
  await validateLinks(articleContents, slugMap);
  printSummary(mdFiles.length);
}

try {
  await main();
} catch (err) {
  console.error("Unexpected error:", err);
  process.exit(1);
}
