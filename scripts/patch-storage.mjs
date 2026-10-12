import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(process.argv[2] ?? 'work/upstream/storage')
const pkgFile = resolve(root, 'package.json')
const pkg = JSON.parse(readFileSync(pkgFile, 'utf8'))
delete pkg.dependencies['fs-xattr']
writeFileSync(pkgFile, JSON.stringify(pkg, null, 2) + '\n')
const lockFile = resolve(root, 'package-lock.json')
const lock = JSON.parse(readFileSync(lockFile, 'utf8'))
delete lock.packages[''].dependencies['fs-xattr']
delete lock.packages['node_modules/fs-xattr']
if (lock.dependencies) delete lock.dependencies['fs-xattr']
writeFileSync(lockFile, JSON.stringify(lock, null, 2) + '\n')
const backend = resolve(root, 'src/storage/backend/file.ts')
let source = readFileSync(backend, 'utf8')
source = source.replace("import * as xattr from 'fs-xattr'", "import * as xattr from './xattr-windows'")
writeFileSync(backend, source)
const backendTest = resolve(root, 'src/storage/backend/file.test.ts')
writeFileSync(backendTest, readFileSync(backendTest, 'utf8').replaceAll("'fs-xattr'", "'./xattr-windows'"))
writeFileSync(resolve(root, 'src/storage/backend/xattr-windows.ts'), `// Native Windows port: NTFS alternate data streams replace POSIX xattrs.
import { readFile, writeFile, stat } from 'node:fs/promises'
function streamPath(file: string, attribute: string): string {
  if (process.platform !== 'win32') throw new Error('Windows metadata adapter requires Windows/NTFS')
  if (!/^[a-zA-Z0-9.-]+$/.test(attribute)) throw new Error('Invalid metadata attribute')
  return file + ':supabase.' + attribute
}
export async function get(file: string, attribute: string): Promise<Buffer | undefined> {
  await stat(file)
  try { return await readFile(streamPath(file, attribute)) }
  catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return undefined; throw error }
}
export async function set(file: string, attribute: string, value: string): Promise<void> {
  await stat(file)
  await writeFile(streamPath(file, attribute), value)
}
`)
const secureFile = resolve(root, 'src/storage/backend/secure-path.ts')
let secure = readFileSync(secureFile, 'utf8')
// Relocate the earlier prototype guard so upstream traversal errors stay stable.
const oldGuard = secure.indexOf("  if (process.platform === 'win32') {")
const traversal = secure.indexOf('  const hasDotTraversalSegment = relativePath')
if (oldGuard >= 0 && oldGuard < traversal && secure.includes('Windows reserved filename')) {
  secure = secure.slice(0, oldGuard) + secure.slice(traversal)
}
if (!secure.includes('Windows reserved filename')) {
  const anchor = '  const normalizedRootPath ='
  if (!secure.includes(anchor)) throw Error('Storage secure-path source changed')
  secure = secure.replace(anchor, `  if (process.platform === 'win32') {
    const segments = relativePath.split(/[\\\\/]+/).filter(Boolean)
    for (const segment of segments) {
      if (/[<>:"|?*\\x00-\\x1f]/.test(segment) || /[. ]$/.test(segment) || /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\\.|$)/i.test(segment)) {
        throwInvalidKey(relativePath, 'Windows reserved filename or alternate-stream syntax')
      }
    }
  }

` + anchor)
  writeFileSync(secureFile, secure)
}
console.log('Storage: removed Unix fs-xattr and added NTFS metadata and reserved-path checks.')
