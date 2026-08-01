#!/usr/bin/env node
// 结构门禁：阻止 Tauri root 依赖绕过、禁止符号回流、无解释规模超限。
// 用法：node scripts/structural-gate.mjs [--list]   （--list 只列出不退出非零）
// 生成物与 vendor 目录一律排除。

import { readFileSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const failFast = !process.argv.includes('--list');

const EXCLUDED_DIRS = new Set([
  'node_modules',
  'build',
  '.svelte-kit',
  'target',
  'dist',
  '.git',
  '.trellis',
  'paraglide', // 生成物（src/lib/paraglide）
  'src/messages', // Paraglide 生成目录
]);

const EXCLUDED_PREFIXES = ['.svelte-kit', 'build/', 'node_modules/', 'target/'];

/** 项目自有源码文件（排除生成物） */
function listSourceFiles() {
  const files = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (EXCLUDED_DIRS.has(entry.name)) continue;
      const full = join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (/\.(rs|ts|svelte|ts\.js)$/.test(entry.name)) files.push(full);
    }
  };
  walk(join(root, 'src'));
  walk(join(root, 'src-tauri'));
  return files;
}

/** Tauri root Cargo.toml 的 [dependencies] 本地 path 依赖 */
function rootLocalDependencies() {
  const manifest = readFileSync(join(root, 'src-tauri', 'Cargo.toml'), 'utf8');
  const inDeps = /^\[dependencies\]$/m.test(manifest);
  if (!inDeps) return [];
  const section = manifest.split(/^\[/m).find((part) => part.startsWith('dependencies]'));
  if (!section) return [];
  const deps = [];
  for (const line of section.split('\n')) {
    const m = line.match(/^([a-z0-9-]+)\s*=\s*\{\s*path\s*=\s*"\.\.\/crates\/([a-z0-9-]+)"/);
    if (m) deps.push(m[2]);
  }
  return deps;
}

/** 禁止符号列表（第三方 authoring 回流、旧编辑器链） */
const FORBIDDEN_SYMBOLS = ['authoring', 'raw_editor', 'language_service', 'document_editing', 'source_document_api'];

/** 文件行数超限的例外清单：<相对路径> -> 原因（审计通过，不可再分单一职责） */
const FILE_SIZE_EXCEPTIONS = new Map([
  // 生产代码：函数均 <50 行、单一职责深模块（前序任务已审计模块边界）
  ['src-tauri/crates/lj-importer/src/strict_json.rs', '严格 JSON 输入边界 + Legado 字段分类表，函数 <50 行，拆分需跨模块数据传递'],
  ['src-tauri/crates/lj-rule-system/src/system/session_delivery.rs', '执行事件投递映射单一 owner，函数 <50 行'],
  ['src-tauri/crates/lj-storage/src/repository/maintenance.rs', '维护事务单一 owner（501 行，超限 1 行），函数 <50 行'],
  // 测试：按 owner contract 组织的 fixture 密集文件，函数数 22-28 个、名称指向不变量
  ['src-tauri/crates/lj-compiler/tests/plan_compiler_test.rs', 'compiler owner contract 测试，28 个函数平均 31 行，fixture 本地专属'],
  ['src-tauri/crates/lj-runtime/tests/plan_runtime_test.rs', 'runtime owner contract 聚合文件，已拆子模块目录 plan_runtime_test/'],
  ['src-tauri/crates/lj-runtime/tests/plan_runtime_test/control_contract.rs', 'runtime control 子合同，fixture 密集'],
  ['src-tauri/crates/lj-storage/tests/event_projection_storage_test.rs', 'storage owner contract 聚合文件，已拆子模块目录'],
  ['src-tauri/crates/lj-storage/tests/event_projection_storage_test/archive_contract.rs', 'storage archive 子合同，durable capture fixture 密集'],
  ['src-tauri/crates/lj-storage/tests/event_projection_storage_test/projection_retention_contract.rs', 'storage projection retention 子合同'],
  ['src-tauri/crates/lj-rule-model/tests/flow_contract_test.rs', 'rule-model flow 合同，fixture 密集'],
  ['src-tauri/crates/lj-node-http/tests/processor_test.rs', 'HTTP adapter owner contract，wiremock fixture 密集'],
  ['src-tauri/crates/lj-integration-tests/tests/legado_rule_system.rs', 'Legado 六个 intent live/replay 黄金合同，场景级断言 helper 密集（已抽取共享 harness 至 test_support.rs）'],
  ['src-tauri/crates/lj-integration-tests/tests/maccms_json_rule_system.rs', 'Maccms 四个 intent + 安全边界集成合同，场景级断言 helper 密集（已抽取共享 harness）'],
]);

const FILE_SIZE_LIMIT = 500;

const violations = [];

// 1. Tauri root 依赖方向
const rootDeps = rootLocalDependencies();
const allowed = new Set(['lj-rule-system']);
for (const dep of rootDeps) {
  if (!allowed.has(dep)) {
    violations.push(`[root-dependency] lanjing 直接依赖 ${dep}，违反唯一业务 facade（只允许 lj-rule-system）`);
  }
}

// 2. 禁止符号
for (const file of listSourceFiles()) {
  const text = readFileSync(file, 'utf8');
  const rel = relative(root, file).replace(/\\/g, '/');
  for (const symbol of FORBIDDEN_SYMBOLS) {
    // 只查标识符形态（避免匹配注释里的说明文字：形如 xxx_yyy 或 camelCase）
    const regex = new RegExp(`\\b${symbol}\\b`, 'i');
    if (regex.test(text)) {
      violations.push(`[forbidden-symbol] ${rel} 含禁止符号 "${symbol}"`);
    }
  }
}

// 3. 文件规模（只统计项目自有 .rs 文件；前端 .svelte/.ts 已有 prettier/eslint 门禁，规模用例外清单登记）
for (const file of listSourceFiles()) {
  if (!file.endsWith('.rs')) continue;
  const rel = relative(root, file).replace(/\\/g, '/');
  if (EXCLUDED_PREFIXES.some((p) => rel.startsWith(p))) continue;
  if (rel.startsWith('crates/') && !rel.startsWith('crates/lj-')) continue;
  const lines = readFileSync(file, 'utf8').split('\n').length;
  if (lines > FILE_SIZE_LIMIT && !FILE_SIZE_EXCEPTIONS.has(rel)) {
    violations.push(`[file-size] ${rel} ${lines} 行 > ${FILE_SIZE_LIMIT}（如需豁免请加入 FILE_SIZE_EXCEPTIONS 并写审计理由）`);
  }
}

if (violations.length > 0) {
  console.error(`结构门禁发现 ${violations.length} 个问题:`);
  for (const v of violations) console.error(`  ${v}`);
  if (failFast) process.exit(1);
} else {
  console.log('结构门禁通过：root 依赖方向、禁止符号、文件规模均合规。');
}
