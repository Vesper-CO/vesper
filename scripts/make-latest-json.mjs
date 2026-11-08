// Builds latest.json, the file Vesper's updater reads, from the signed NSIS installer Tauri just made.
// Usage: node scripts/make-latest-json.mjs <tag> <owner/repo> [notes]
// Writes ./latest.json and prints the files that belong on the release.
import { readFileSync, readdirSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const [tag, repo, notes = ''] = process.argv.slice(2);
const fail = (m) => { console.error(`::error::${m}`); process.exit(1); };
if (!tag || !repo) fail('usage: make-latest-json.mjs <tag> <owner/repo> [notes]');

const conf = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
const version = conf.version;
if (tag.replace(/^v/, '') !== version) {
  fail(`The tag ${tag} does not match the version ${version} in src-tauri/tauri.conf.json. Bump the version there (and in Cargo.toml and package.json), then tag again.`);
}

const dir = process.env.NSIS_DIR || 'src-tauri/target/release/bundle/nsis';
if (!existsSync(dir)) fail(`${dir} does not exist: the installer was not built.`);
const exe = readdirSync(dir).find((f) => f.endsWith('-setup.exe'));
if (!exe) fail(`No *-setup.exe in ${dir}.`);
const sigFile = join(dir, `${exe}.sig`);
if (!existsSync(sigFile)) {
  fail(`${exe}.sig was not created, so the build was not signed. Set the TAURI_SIGNING_PRIVATE_KEY secret (the full text of your vesper.key file) and TAURI_SIGNING_PRIVATE_KEY_PASSWORD (empty if the key has none).`);
}
const signature = readFileSync(sigFile, 'utf8').trim();
if (!signature) fail(`${exe}.sig is empty.`);

const latest = {
  version,
  notes: notes || `Vesper ${tag}`,
  pub_date: new Date().toISOString().replace(/\.\d+Z$/, 'Z'),
  platforms: {
    'windows-x86_64': {
      signature,
      url: `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(exe)}`,
    },
  },
};
writeFileSync('latest.json', JSON.stringify(latest, null, 2) + '\n');
console.log(`latest.json for ${version} -> ${latest.platforms['windows-x86_64'].url}`);
console.log(`FILES=${join(dir, exe)}|${sigFile}|latest.json`);
