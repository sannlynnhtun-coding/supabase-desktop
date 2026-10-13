import { readFileSync, mkdirSync, writeFileSync, existsSync } from 'node:fs'
import { join, resolve, sep } from 'node:path'
import { tmpdir } from 'node:os'
import { randomUUID } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import assert from 'node:assert/strict'

if (process.platform !== 'win32') throw Error('This check requires native Windows')
const root = join(tmpdir(), 'SupabaseLocal-Uninstall-' + randomUUID().replaceAll('-', ''))
assert(resolve(root).startsWith(resolve(tmpdir()) + sep))
mkdirSync(root)
const line = readFileSync('src/SupabaseLocal.App/UninstallWindow.cs', 'utf8').split('\n').find(line => line.includes('File.WriteAllText(script, '))
assert(line, 'Missing cleanup script')
const script = JSON.parse(line.slice(line.indexOf(', ') + 2, line.lastIndexOf(');')))
writeFileSync(join(root, 'cleanup.ps1'), script)
writeFileSync(join(root, 'fixture.txt'), 'private temporary uninstall worker')
const result = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', join(root, 'cleanup.ps1'), '-WorkerId', '2147483647', '-Folder', root], { windowsHide: true, encoding: 'utf8' })
assert.equal(result.status, 0, result.stderr)
assert.equal(existsSync(root), false, 'Temporary worker directory was retained')
console.log('PASS: actual deferred-uninstall PowerShell script removes its checked private worker folder.')
