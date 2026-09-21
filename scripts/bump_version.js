#!/usr/bin/env node
/**
 * Bump version across the version sources:
 *   - package.json
 *   - src-tauri/tauri.conf.json
 *   - src-tauri/Cargo.toml   (only the [package] version line)
 *   - android/app/build.gradle.kts (versionName)
 *
 * 用于 CI：根据发布标签注入版本号，无需本地提交版本号变更。
 * 使用 ESM 语法：项目 package.json 含 "type": "module"，
 * .js 会被当作 ES module，不能用 require()。
 *
 * 用法:
 *   node scripts/bump_version.js 0.1.1
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error('ERROR: version must be X.Y.Z, got:', version || '(none)');
  process.exit(1);
}

// ESM 下没有 __dirname，从 import.meta.url 推导项目根目录
const __dirname = path.dirname(fileURLToPath(import.meta.url));
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

// 安卓端 versionName。versionCode 由 CI 用 run_number 注入（必须单调递增，
// 且与发布次数挂钩），不在这里处理。
const gradlePath = path.join(root, 'android/app/build.gradle.kts');
let gradle = fs.readFileSync(gradlePath, 'utf8');
const gradleRe = /versionName\s*=\s*"[^"]+"/;
if (!gradleRe.test(gradle)) {
  console.error('ERROR: could not find versionName in android/app/build.gradle.kts');
  process.exit(1);
}
gradle = gradle.replace(gradleRe, `versionName = "${version}"`);
fs.writeFileSync(gradlePath, gradle);
console.log(`android/app/build.gradle.kts: -> ${version}`);
