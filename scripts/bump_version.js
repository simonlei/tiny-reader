#!/usr/bin/env node
/**
 * Bump version across the three version sources:
 *   - package.json
 *   - src-tauri/tauri.conf.json
 *   - src-tauri/Cargo.toml   (only the [package] version line)
 *
 * 用于 CI：根据发布标签注入版本号，无需本地提交版本号变更。
 * Node 跨平台，在 GitHub Actions 的 Windows/macOS/Linux runner 上均可运行。
 *
 * 用法:
 *   node scripts/bump_version.js 0.1.1
 */
const fs = require('fs');
const path = require('path');

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error('ERROR: version must be X.Y.Z, got:', version || '(none)');
  process.exit(1);
}

const root = path.resolve(__dirname, '..');

function bumpJson(rel) {
  const p = path.join(root, rel);
  const j = JSON.parse(fs.readFileSync(p, 'utf8'));
  const prev = j.version;
  j.version = version;
  fs.writeFileSync(p, JSON.stringify(j, null, 2) + '\n');
  console.log(`${rel}: ${prev} -> ${version}`);
}

bumpJson('package.json');
bumpJson('src-tauri/tauri.conf.json');

// Cargo.toml 是 TOML，用正则只替换 [package] 下的 version 行。
const cargoPath = path.join(root, 'src-tauri/Cargo.toml');
let cargo = fs.readFileSync(cargoPath, 'utf8');
const cargoRe = /^version\s*=\s*"[^"]+"/m;
if (!cargoRe.test(cargo)) {
  console.error('ERROR: could not find version line in src-tauri/Cargo.toml');
  process.exit(1);
}
cargo = cargo.replace(cargoRe, `version = "${version}"`);
fs.writeFileSync(cargoPath, cargo);
console.log(`src-tauri/Cargo.toml: -> ${version}`);
