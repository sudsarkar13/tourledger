import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { manifest } from "./latest-json.mjs";

const NAMES = [
  "TourLedger.app.tar.gz",
  "TourLedger_0.1.2_x64-setup.exe",
  "TourLedger_0.1.2_x64_en-US.msi",
  "TourLedger_0.1.2_arm64-setup.exe",
  "TourLedger_0.1.2_amd64.AppImage",
  "TourLedger_0.1.2_amd64.deb",
  "TourLedger-0.1.2-1.x86_64.rpm",
  "TourLedger_0.1.2_arm64.deb",
  "TourLedger-0.1.2-1.aarch64.rpm",
];

// Shaped like minisign's files; the real signing is the Tauri tool's work.
const wrap = (...text) => Buffer.from(text.join("\n")).toString("base64");
const line = (id) => Buffer.concat([Buffer.from("Ed"), Buffer.from(id), Buffer.alloc(32)]).toString("base64");
const PUBLIC_KEY = wrap("untrusted comment: public key", line("OURKEY01"));
const signatureOf = (name, id = "OURKEY01", version = "0.1.2") =>
  wrap("untrusted comment: signature", line(id), `trusted comment: timestamp:1\tfile:${name}\tversion:${version}`, "x");

function release(names, unsigned = [], sign = signatureOf) {
  const folder = mkdtempSync(join(tmpdir(), "latest-"));
  mkdirSync(join(folder, "nested"));
  for (const name of names) {
    writeFileSync(join(folder, "nested", name), "installer");
    if (!unsigned.includes(name)) writeFileSync(join(folder, "nested", `${name}.sig`), `${sign(name)}\n`);
  }
  // Installers people download but that never serve as updates.
  writeFileSync(join(folder, "TourLedger_0.1.2_universal.dmg"), "disk image");
  return folder;
}

test("every system points at its own signed file in this release", () => {
  const folder = release(NAMES);
  const result = manifest(folder, "v0.1.2", "First paragraph\nof the notes.\n\n## More\n", PUBLIC_KEY);
  rmSync(folder, { recursive: true });
  assert.equal(result.version, "0.1.2");
  assert.equal(result.notes, "First paragraph of the notes.");
  assert.equal(Object.keys(result.platforms).length, 13);
  assert.equal(result.platforms["darwin-x86_64"].url, result.platforms["darwin-aarch64"].url);
  assert.equal(
    result.platforms["linux-aarch64-rpm"].url,
    "https://github.com/sudsarkar13/tourledger/releases/download/v0.1.2/TourLedger-0.1.2-1.aarch64.rpm",
  );
  assert.equal(result.platforms["windows-x86_64-msi"].signature, signatureOf("TourLedger_0.1.2_x64_en-US.msi"));
});

test("a missing file or signature stops the release", () => {
  const missing = release(NAMES.slice(1));
  assert.throws(() => manifest(missing, "v0.1.2", "n", PUBLIC_KEY), /found 0/);
  const unsigned = release(NAMES, ["TourLedger_0.1.2_amd64.deb"]);
  assert.throws(() => manifest(unsigned, "v0.1.2", "n", PUBLIC_KEY), /No signature for TourLedger_0\.1\.2_amd64\.deb/);
  rmSync(missing, { recursive: true });
  rmSync(unsigned, { recursive: true });
});

test("a signature the apps would refuse stops the release", () => {
  const otherKey = release(NAMES, [], (name) => signatureOf(name, "SOMEONE1"));
  assert.throws(() => manifest(otherKey, "v0.1.2", "n", PUBLIC_KEY), /another key/);
  const otherVersion = release(NAMES, [], (name) => signatureOf(name, "OURKEY01", "0.1.1"));
  assert.throws(() => manifest(otherVersion, "v0.1.2", "n", PUBLIC_KEY), /signed for version 0\.1\.1/);
  rmSync(otherKey, { recursive: true });
  rmSync(otherVersion, { recursive: true });
});
