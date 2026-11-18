// Copies the fonts and icons the Rust renderer embeds into render/assets.
// Run from the repo root after `npm ci`:  node render/tools/prepare-assets.mjs
// The output is committed, so the server never needs to run this.
//
// Fonts: Barlow Condensed (SIL OFL) from @fontsource. fontsource ships WOFF 1.0, which is a
// zlib-compressed TrueType file; this unpacks it back into a plain .ttf the renderer can read.
// Icons: Remix Icon (Apache 2.0) SVGs from the remixicon package.
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "../..");
const out = path.join(root, "render", "assets");
fs.mkdirSync(path.join(out, "fonts"), { recursive: true });
fs.mkdirSync(path.join(out, "icons"), { recursive: true });

function woffToTtf(buf) {
  if (buf.toString("ascii", 0, 4) !== "wOFF") throw new Error("not a WOFF 1.0 file");
  const flavor = buf.readUInt32BE(4);
  const numTables = buf.readUInt16BE(12);
  const tables = [];
  for (let i = 0; i < numTables; i++) {
    const o = 44 + i * 20;
    const tag = buf.toString("ascii", o, o + 4);
    const offset = buf.readUInt32BE(o + 4);
    const compLength = buf.readUInt32BE(o + 8);
    const origLength = buf.readUInt32BE(o + 12);
    const checksum = buf.readUInt32BE(o + 16);
    const raw = buf.subarray(offset, offset + compLength);
    const data = compLength < origLength ? zlib.inflateSync(raw) : Buffer.from(raw);
    tables.push({ tag, checksum, data });
  }
  let entrySelector = 0;
  while (1 << (entrySelector + 1) <= numTables) entrySelector++;
  const searchRange = (1 << entrySelector) * 16;
  const header = Buffer.alloc(12 + numTables * 16);
  header.writeUInt32BE(flavor, 0);
  header.writeUInt16BE(numTables, 4);
  header.writeUInt16BE(searchRange, 6);
  header.writeUInt16BE(entrySelector, 8);
  header.writeUInt16BE(numTables * 16 - searchRange, 10);
  let offset = header.length;
  const bodies = [];
  tables.forEach((t, i) => {
    const o = 12 + i * 16;
    header.write(t.tag, o, "ascii");
    header.writeUInt32BE(t.checksum, o + 4);
    header.writeUInt32BE(offset, o + 8);
    header.writeUInt32BE(t.data.length, o + 12);
    const padded = Buffer.alloc((t.data.length + 3) & ~3);
    t.data.copy(padded);
    bodies.push(padded);
    offset += padded.length;
  });
  return Buffer.concat([header, ...bodies]);
}

const fontDir = path.join(root, "node_modules/@fontsource/barlow-condensed/files");
for (const weight of [500, 600, 700, 800, 900]) {
  const src = path.join(fontDir, `barlow-condensed-latin-${weight}-normal.woff`);
  const ttf = woffToTtf(fs.readFileSync(src));
  fs.writeFileSync(path.join(out, "fonts", `BarlowCondensed-${weight}.ttf`), ttf);
}

const iconDir = path.join(root, "node_modules/remixicon/icons");
const icons = {
  Weather: [
    "sun-fill", "moon-clear-fill", "sun-cloudy-fill", "moon-cloudy-fill", "cloudy-fill", "cloudy-2-fill",
    "rainy-fill", "showers-fill", "heavy-showers-fill", "drizzle-fill", "thunderstorms-fill", "snowy-fill",
    "snowflake-fill", "hail-fill", "foggy-fill", "mist-fill", "haze-2-fill", "windy-fill", "tornado-fill",
    "temp-hot-fill", "temp-cold-fill", "flood-fill", "fire-fill", "typhoon-fill", "sparkling-fill",
    "cloud-windy-fill", "sun-foggy-fill", "moon-foggy-fill",
  ],
  System: ["alert-fill", "error-warning-fill", "information-fill", "checkbox-circle-fill", "time-fill", "shield-check-fill"],
  Map: ["map-pin-2-fill", "radar-fill", "signal-tower-fill"],
  Business: ["flag-fill"],
  Others: ["gift-fill"],
  Device: ["radar-fill"],
};
const found = new Set();
for (const [folder, names] of Object.entries(icons)) {
  for (const name of names) {
    if (found.has(name)) continue;
    const file = path.join(iconDir, folder, `${name}.svg`);
    if (!fs.existsSync(file)) continue;
    fs.copyFileSync(file, path.join(out, "icons", `${name}.svg`));
    found.add(name);
  }
}
// Anything not in the folder we guessed: search every folder.
const wanted = Object.values(icons).flat().filter((n) => !found.has(n));
for (const folder of fs.readdirSync(iconDir)) {
  for (const name of wanted) {
    const file = path.join(iconDir, folder, `${name}.svg`);
    if (!found.has(name) && fs.existsSync(file)) {
      fs.copyFileSync(file, path.join(out, "icons", `${name}.svg`));
      found.add(name);
    }
  }
}
const missing = Object.values(icons).flat().filter((n) => !found.has(n));
console.log(`fonts: 5, icons: ${found.size}${missing.length ? `, missing: ${[...new Set(missing)].join(", ")}` : ""}`);

const brand = path.join(root, "public/assets/icons/Brandkit/DuboisWXTransparentOptimized.svg");
if (fs.existsSync(brand)) fs.copyFileSync(brand, path.join(out, "brand-logo.svg"));
