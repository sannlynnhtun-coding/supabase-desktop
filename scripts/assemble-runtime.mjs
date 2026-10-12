import { copyFileSync, existsSync, mkdirSync, writeFileSync, readlinkSync, statSync, lstatSync, readdirSync } from 'node:fs'
import { dirname, resolve, join, relative, sep } from 'node:path'

const destination = resolve(process.argv[2] ?? 'artifacts/runtime')
if (existsSync(destination)) throw Error('Use a fresh runtime destination to avoid mixing versions.')
const upstream = resolve('work/upstream')
const runtime = resolve('work/runtime')
const standalone = resolve(upstream, 'supabase/apps/studio/.next/standalone')
if (!existsSync(resolve(standalone, 'apps/studio/server.js'))) throw Error('Studio standalone build is not ready.')
mkdirSync(destination, { recursive: true })
const links = {}
function copy(source, target) {
  const output = resolve(destination, target)
  materialize(source, output, resolve(source), output)
}
function materialize(source, target, sourceRoot, outputRoot) {
  if (lstatSync(source).isSymbolicLink()) {
    let original = resolve(dirname(source), readlinkSync(source))
    let rel = relative(sourceRoot, original)
    if (rel.startsWith('..') && sourceRoot === standalone) {
      // Turbopack emits absolute Windows links back into the build checkout.
      // Relocate those to the corresponding traced standalone package.
      const fromRepo = relative(resolve(upstream, 'supabase'), original)
      if (fromRepo.startsWith('..')) throw Error('Runtime link escapes upstream source: ' + source)
      original = resolve(standalone, fromRepo)
      rel = relative(sourceRoot, original)
    }
    if (rel.startsWith('..')) throw Error('Runtime link escapes its tree: ' + source)
    if (!existsSync(original)) return // untraced pnpm peers are unused dangling links
    links[relative(destination, target).split(sep).join('/')] = {
      target: relative(destination, resolve(outputRoot, rel)).split(sep).join('/'),
      directory: statSync(original).isDirectory(),
    }
    return
  }
  if (!statSync(source).isDirectory()) {
    mkdirSync(dirname(target), { recursive: true })
    copyFileSync(source, target)
    return
  }
  mkdirSync(target, { recursive: true })
  for (const name of readdirSync(source)) materialize(join(source, name), join(target, name), sourceRoot, outputRoot)
}
// Preserve pnpm's package graph without recursively copying its cyclic links.
// Setup restores directory links as NTFS junctions (no admin permission needed).
copy(standalone, 'studio')
copy(resolve(upstream, 'supabase/apps/studio/.next/static'), 'studio/apps/studio/.next/static')
copy(resolve(upstream, 'supabase/apps/studio/public'), 'studio/apps/studio/public')
writeFileSync(resolve(destination, 'studio/server.js'), "require('./apps/studio/server.js')\n")
for (const directory of ['bin', 'lib', 'share']) copy(resolve(runtime, 'pgsql', directory), 'postgres/' + directory)
copy(resolve(runtime, 'node-v22.16.0-win-x64/node.exe'), 'node/node.exe')
copy(resolve(runtime, 'node-v22.16.0-win-x64/LICENSE'), 'licenses/node-LICENSE.txt')
copy(resolve(runtime, 'node-v24.15.0-win-x64/node.exe'), 'node-storage/node.exe')
copy(resolve(runtime, 'node-v24.15.0-win-x64/LICENSE'), 'licenses/node-storage-LICENSE.txt')
copy(resolve(runtime, 'storage'), 'storage')
copy(resolve(runtime, 'storage/LICENSE'), 'licenses/storage-LICENSE.txt')
copy(resolve(upstream, 'postgres-meta/dist'), 'pgmeta/dist')
copy(resolve(upstream, 'postgres-meta/node_modules'), 'pgmeta/node_modules')
copy(resolve(upstream, 'postgres-meta/package.json'), 'pgmeta/package.json')
copy(resolve(upstream, 'postgres-meta/LICENSE'), 'licenses/postgres-meta-LICENSE.txt')
copy(resolve(runtime, 'auth/auth.exe'), 'auth/auth.exe')
copy(resolve(upstream, 'auth/LICENSE'), 'licenses/auth-LICENSE.txt')
copy(resolve(runtime, 'postgrest/postgrest.exe'), 'postgrest/postgrest.exe')
copy(resolve('third-party/postgrest-LICENSE.txt'), 'licenses/postgrest-LICENSE.txt')
copy(resolve('third-party/postgres-COPYRIGHT.txt'), 'licenses/postgres-COPYRIGHT.txt')
copy(resolve(upstream, 'supabase/LICENSE'), 'licenses/supabase-LICENSE.txt')
copy(resolve('scripts/sources.lock.json'), 'sources.lock.json')
writeFileSync(resolve(destination, 'licenses/NOTICE.txt'), 'Unofficial experimental native Windows integration. Upstream services retain their licenses. PostgreSQL copyright and extension notices are included under postgres/share. PostgREST is distributed under the MIT license: https://github.com/PostgREST/postgrest/blob/v16.4/LICENSE\n')
writeFileSync(resolve(destination, 'links.json'), JSON.stringify(links, null, 2) + '\n')
console.log('Assembled native Windows runtime at ' + destination)
