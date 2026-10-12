import { copyFileSync, mkdirSync, lstatSync, readdirSync } from 'node:fs'
import { resolve, join, dirname } from 'node:path'
const source = resolve(process.argv[2])
const destination = resolve(process.argv[3])
function walk(from, to) {
  const info = lstatSync(from)
  if (info.isSymbolicLink()) return // links.json restores these after installation
  if (info.isDirectory()) {
    mkdirSync(to, { recursive: true })
    for (const name of readdirSync(from)) walk(join(from, name), join(to, name))
  } else { mkdirSync(dirname(to), { recursive: true }); copyFileSync(from, to) }
}
walk(source, destination)
