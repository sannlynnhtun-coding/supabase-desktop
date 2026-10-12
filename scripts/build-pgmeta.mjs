// Windows cmd.exe preserves single quotes in npm scripts. Copy assets with Node
// rather than executing upstream's POSIX-quoted cpy command.
import { spawnSync } from 'node:child_process'
import { cpSync, existsSync, mkdirSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(process.argv[2] ?? 'work/upstream/postgres-meta')
if (!existsSync(resolve(root, 'src/server/server.ts'))) throw Error('Expected postgres-meta source')
const result = spawnSync(process.execPath, [resolve(root, 'node_modules/typescript/bin/tsc'), '-p', 'tsconfig.json'], {
  cwd: root, stdio: 'inherit', shell: false,
})
if (result.error) throw result.error
if (result.status !== 0) process.exit(result.status ?? 1)
mkdirSync(resolve(root, 'dist/lib/sql'), { recursive: true })
cpSync(resolve(root, 'src/lib/sql'), resolve(root, 'dist/lib/sql'), { recursive: true })
cpSync(resolve(root, 'src/server/format-worker.js'), resolve(root, 'dist/server/format-worker.js'))
console.log('postgres-meta compiled and SQL/worker assets copied for Windows.')
