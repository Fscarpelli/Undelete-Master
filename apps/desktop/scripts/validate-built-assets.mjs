import { access, readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { stdout } from "node:process";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const distRoot = resolve(appRoot, "dist");
const html = await readFile(resolve(distRoot, "index.html"), "utf8");

function fail(message) {
  throw new Error(`Built asset validation failed: ${message}`);
}

function localAssetPath(reference, extension) {
  if (
    !(
      reference.startsWith("/assets/") ||
      reference.startsWith("./assets/")
    ) ||
    !reference.endsWith(extension)
  ) {
    fail(`expected a same-origin ${extension} asset, received ${reference}`);
  }
  return resolve(distRoot, reference.replace(/^\.?\//u, ""));
}

if (/<style(?:\s|>)/iu.test(html) || /\sstyle\s*=/iu.test(html)) {
  fail("inline styles are present in dist/index.html");
}
if (html.includes("'unsafe-inline'") || html.includes('"unsafe-inline"')) {
  fail("unsafe-inline is present in dist/index.html");
}

const stylesheetTags =
  html.match(/<link\b[^>]*\brel=["']stylesheet["'][^>]*>/giu) ?? [];
if (stylesheetTags.length !== 1) {
  fail(`expected one external stylesheet, received ${stylesheetTags.length}`);
}
const stylesheetReference =
  stylesheetTags[0]?.match(/\bhref=["']([^"']+)["']/iu)?.[1];
if (!stylesheetReference) {
  fail("the stylesheet link has no href");
}
await access(localAssetPath(stylesheetReference, ".css"));

const moduleScriptTags =
  html.match(/<script\b[^>]*\btype=["']module["'][^>]*><\/script>/giu) ?? [];
if (moduleScriptTags.length !== 1) {
  fail(`expected one external module script, received ${moduleScriptTags.length}`);
}
const scriptReference =
  moduleScriptTags[0]?.match(/\bsrc=["']([^"']+)["']/iu)?.[1];
if (!scriptReference) {
  fail("the module script has no src");
}
await access(localAssetPath(scriptReference, ".js"));

stdout.write(
  "Built asset validation passed: external same-origin CSS and JavaScript only.\n",
);
