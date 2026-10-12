import { readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { resolve, join, relative, sep } from 'node:path'
const root = resolve(process.argv[2])
const files = {}
function walk(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name)
    if (entry.isSymbolicLink()) throw Error('Payload contains an unmaterialized link: ' + path)
    if (entry.isDirectory()) walk(path)
    else if (entry.isFile() && path !== join(root, 'manifest.json')) {
      files[relative(root, path).split(sep).join('/')] = createHash('sha256').update(readFileSync(path)).digest('hex')
    }
  }
}
walk(root)
writeFileSync(join(root, 'manifest.json'), JSON.stringify(files, null, 2) + '\n')
console.log('Verified manifest: ' + Object.keys(files).length + ' files')
