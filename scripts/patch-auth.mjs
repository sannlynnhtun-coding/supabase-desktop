import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(process.argv[2] ?? 'work/upstream/auth')
const file = resolve(root, 'cmd/serve_cmd.go')
let source = readFileSync(file, 'utf8')
if (source.includes('lc := platformListenConfig()')) process.exit(0)
const start = source.indexOf('\tlc := net.ListenConfig{')
const end = source.indexOf('\n\tlistener, err :=', start)
if (start < 0 || end < 0) throw Error('Auth source changed: expected listener setup not found')
const control = source.slice(start, end).replace('lc := ', 'return ')
source = source.slice(0, start) + '\tlc := platformListenConfig()' + source.slice(end)
source = source.replace(/^\s*"syscall"\r?\n/m, '\n').replace(/^\s*"golang.org\/x\/sys\/unix"\r?\n/m, '\n')
writeFileSync(file, source)
writeFileSync(resolve(root, 'cmd/listen_unix.go'), `//go:build !windows\n\npackage cmd\n\nimport ("net"; "syscall"; "golang.org/x/sys/unix")\n\nfunc platformListenConfig() net.ListenConfig {\n${control}\n}\n`)
writeFileSync(resolve(root, 'cmd/listen_windows.go'), `package cmd\n\nimport "net"\n\n// Windows uses an exclusive listener. SO_REUSEPORT has no equivalent here;\n// concurrent Auth instances must bind different ports.\nfunc platformListenConfig() net.ListenConfig { return net.ListenConfig{} }\n`)
console.log('Auth: isolated Unix SO_REUSEPORT control and added Windows listener.')
