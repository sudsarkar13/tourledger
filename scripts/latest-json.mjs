// Writes latest.json, the list installed desktop apps read to find an update.
//
//   node scripts/latest-json.mjs <folder with installers> <tag> <notes file>
//
// It fails rather than leave a system out: a missing entry would not be noticed,
// that system would simply never update. It also fails on a signature the apps
// would refuse: from another key, or naming another version.
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { pathToFileURL } from "node:url";

const REPOSITORY = "sudsarkar13/tourledger";

/** The updater's name for each system and kind of install, and the file that serves it. */
const TARGETS = [
  { keys: ["darwin-aarch64", "darwin-x86_64"], file: /\.app\.tar\.gz$/ },
  // The plain key is used when the app cannot tell how it was installed.
  { keys: ["windows-x86_64-nsis", "windows-x86_64"], file: /_x64-setup\.exe$/ },
  { keys: ["windows-x86_64-msi"], file: /_x64_en-US\.msi$/ },
  { keys: ["windows-aarch64-nsis", "windows-aarch64"], file: /_arm64-setup\.exe$/ },
  { keys: ["linux-x86_64-appimage", "linux-x86_64"], file: /_amd64\.AppImage$/ },
  { keys: ["linux-x86_64-deb"], file: /_amd64\.deb$/ },
  { keys: ["linux-x86_64-rpm"], file: /\.x86_64\.rpm$/ },
  { keys: ["linux-aarch64-deb"], file: /_arm64\.deb$/ },
  { keys: ["linux-aarch64-rpm"], file: /\.aarch64\.rpm$/ },
];

const files = (folder) =>
  readdirSync(folder, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => join(entry.parentPath, entry.name));

// Both are base64 text around a minisign file whose second line holds the key's
// 8-byte id after a 2-byte algorithm name.
const lines = (text) => Buffer.from(text.trim(), "base64").toString("utf8").split("\n");
const keyId = (text) => Buffer.from(lines(text)[1] ?? "", "base64").subarray(2, 10).toString("hex");

function check(name, signature, publicKey, version) {
  if (keyId(signature) !== keyId(publicKey)) {
    throw new Error(`${name} is signed with another key than the apps trust`);
  }
  const signed = lines(signature)
    .find((line) => line.startsWith("trusted comment:"))
    ?.split("\t")
    .find((field) => field.startsWith("version:"))
    ?.slice("version:".length)
    .trim();
  if (signed !== version) throw new Error(`${name} is signed for version ${signed ?? "(none)"}, not ${version}`);
}

export function manifest(folder, tag, notes, publicKey, now = new Date()) {
  const version = tag.replace(/^v/, "");
  const all = files(folder);
  const platforms = {};
  for (const target of TARGETS) {
    const found = all.filter((path) => target.file.test(path));
    if (found.length !== 1) throw new Error(`Expected one file matching ${target.file}, found ${found.length}`);
    const signature = all.find((path) => path === `${found[0]}.sig`);
    if (!signature) throw new Error(`No signature for ${basename(found[0])}: apps would refuse it`);
    const signed = readFileSync(signature, "utf8").trim();
    check(basename(found[0]), signed, publicKey, version);
    const entry = {
      signature: signed,
      url: `https://github.com/${REPOSITORY}/releases/download/${tag}/${encodeURIComponent(basename(found[0]))}`,
    };
    for (const key of target.keys) platforms[key] = entry;
  }
  return {
    version,
    // The first paragraph of the release notes.
    notes: notes.trim().split(/\n\s*\n/)[0].replace(/\s+/g, " "),
    pub_date: now.toISOString(),
    platforms,
  };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const [folder, tag, notes] = process.argv.slice(2);
  if (!folder || !tag || !notes) {
    console.error("Usage: node scripts/latest-json.mjs <folder> <tag> <notes file>");
    process.exit(1);
  }
  const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
  if (`v${config.version}` !== tag) throw new Error(`The app is version ${config.version}, the tag is ${tag}`);
  const result = manifest(folder, tag, readFileSync(notes, "utf8"), config.plugins.updater.pubkey);
  writeFileSync("latest.json", JSON.stringify(result, null, 2) + "\n");
  console.log(`latest.json: ${result.version}, ${Object.keys(result.platforms).length} targets`);
}
