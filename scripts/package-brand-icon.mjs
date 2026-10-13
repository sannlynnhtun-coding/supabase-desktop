import { readFileSync, writeFileSync } from 'node:fs'
import assert from 'node:assert/strict'

// Container conversion only: preserve the official, pre-sized PNG bytes verbatim.
const frames = [16, 32, 48, 96, 128, 196].map(size => {
  const name = size === 128 ? 'favicon-128.png' : `favicon-${size}x${size}.png`
  const png = readFileSync('work/upstream/supabase/apps/studio/public/favicon/' + name)
  assert.equal(png.readUInt32BE(16), size)
  assert.equal(png.readUInt32BE(20), size)
  return { size, png }
})
const header = Buffer.alloc(6 + 16 * frames.length)
header.writeUInt16LE(1, 2); header.writeUInt16LE(frames.length, 4)
let offset = header.length
frames.forEach(({ size, png }, index) => {
  const start = 6 + index * 16
  header[start] = size; header[start + 1] = size
  header.writeUInt16LE(1, start + 4); header.writeUInt16LE(32, start + 6)
  header.writeUInt32LE(png.length, start + 8); header.writeUInt32LE(offset, start + 12)
  offset += png.length
})
writeFileSync('src/Shared/Branding/supabase.ico', Buffer.concat([header, ...frames.map(frame => frame.png)]))
console.log('Packed six unchanged official Supabase PNG sizes into the Windows icon.')
