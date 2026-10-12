import { spawnSync } from 'node:child_process'
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

const studio = resolve(process.argv[2] ?? 'work/upstream/supabase/apps/studio')
const cli = resolve(studio, 'node_modules/next/dist/bin/next')
if (!existsSync(cli)) throw Error('Install the pinned Studio workspace dependencies first.')
const configPath = resolve(studio, 'next.config.ts')
let config = readFileSync(configPath, 'utf8')
if (!config.includes('cpus: 2,')) {
  if (!config.includes('experimental: {')) throw Error('Studio Next configuration changed')
  config = config.replace('experimental: {', 'experimental: {\n    cpus: 2, // Bound static-generation memory on Windows build machines.')
  writeFileSync(configPath, config)
}
// Invoke Next directly: upstream build/start wrappers contain POSIX syntax.
const result = spawnSync(process.execPath, [cli, 'build'], {
  cwd: studio, stdio: 'inherit', shell: false,
  env: { ...process.env, NEXT_PUBLIC_IS_PLATFORM: 'false', FORCE_ASSET_CDN: '-1',
    NEXT_TELEMETRY_DISABLED: '1', NODE_OPTIONS: '--max-old-space-size=8192' },
})
if (result.error) throw result.error
process.exit(result.status ?? 1)
