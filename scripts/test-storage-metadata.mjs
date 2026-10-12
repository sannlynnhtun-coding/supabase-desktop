import { createRequire } from 'node:module'
import { readFileSync, writeFileSync, mkdirSync, rmSync, copyFileSync, existsSync } from 'node:fs'
import { resolve } from 'node:path'
import assert from 'node:assert/strict'

if (process.platform !== 'win32') throw Error('Run this integration check on Windows/NTFS')
const root = resolve('work/storage-metadata-check-' + Date.now())
mkdirSync(root, { recursive: true })
const require = createRequire(import.meta.url)
const ts = require(resolve('work/upstream/postgres-meta/node_modules/typescript'))
const source = readFileSync(resolve('work/upstream/storage/src/storage/backend/xattr-windows.ts'), 'utf8')
const output = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText
writeFileSync(resolve(root, 'adapter.cjs'), output)
const adapter = require(resolve(root, 'adapter.cjs'))
const file = resolve(root, 'object.txt')
try {
  writeFileSync(file, 'native object bytes')
  assert.equal(await adapter.get(file, 'user.supabase.content-type'), undefined)
  await adapter.set(file, 'user.supabase.content-type', 'text/plain')
  await adapter.set(file, 'user.supabase.cache-control', 'max-age=3600')
  assert.equal((await adapter.get(file, 'user.supabase.content-type')).toString(), 'text/plain')
  assert.equal(readFileSync(file, 'utf8'), 'native object bytes')
  await assert.rejects(() => adapter.set(file, 'malicious:stream', 'value'), /Invalid metadata attribute/)
  await assert.rejects(() => adapter.set(resolve(root, 'missing.txt'), 'user.supabase.etag', 'value'), /ENOENT/)
  assert.equal(existsSync(resolve(root, 'missing.txt')), false)
  copyFileSync(file, resolve(root, 'copy.txt'))
  assert.equal((await adapter.get(resolve(root, 'copy.txt'), 'user.supabase.content-type')).toString(), 'text/plain')
  console.log('PASS: NTFS metadata write/read, missing attribute, unchanged object bytes, copy retention and invalid-stream rejection.')
} finally { rmSync(root, { recursive: true }) }
