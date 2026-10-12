import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { createHmac } from 'node:crypto'
import assert from 'node:assert/strict'

const secret = JSON.parse(readFileSync(join(process.argv[2], 'secrets.json'), 'utf8')).JwtSecret
const cycle = Number(process.argv[3])
function key(role) {
  const encode = value => Buffer.from(JSON.stringify(value)).toString('base64url')
  const body = encode({ alg: 'HS256', typ: 'JWT' }) + '.' + encode({ role, iss: 'supabase', iat: 0, exp: 4102444800 })
  return body + '.' + createHmac('sha256', secret).update(body).digest('base64url')
}
const service = key('service_role'), anon = key('anon'), user = process.env.STORAGE_TEST_ACCESS_TOKEN
const base = 'http://127.0.0.1:54321/storage/v1'
async function request(path, { token = service, method = 'GET', body, headers = {}, allowed = [200] } = {}) {
  const response = await fetch(base + path, { method, headers: { apikey: anon, ...(token ? { Authorization: 'Bearer ' + token } : {}), ...headers }, body })
  assert(allowed.includes(response.status), `${method} ${path}: ${response.status} ${await response.clone().text()}`)
  return response
}
async function json(path, method, body, token = service, allowed = [200]) {
  return request(path, { method, token, body: JSON.stringify(body), headers: { 'Content-Type': 'application/json' }, allowed })
}
const content = 'Native Windows Storage — persistence ✓\n'
const privateBucket = 'native-private', publicBucket = 'native-public'
assert.equal((await (await request('/health')).json()).healthy, true)
await request('/bucket', { token: service.slice(0, -1) + (service.endsWith('A') ? 'B' : 'A'), allowed: [400, 401, 403] })
const privilege = await fetch('http://127.0.0.1:54323/api/platform/pg-meta/default/query', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ query: "SELECT pg_has_role('authenticator', 'postgres', 'MEMBER') AS elevated;" }) })
assert(privilege.ok)
assert.equal((await privilege.json())[0].elevated, false, 'Storage migrations elevated the REST role')
if (cycle === 1) {
  for (const [id, isPublic] of [[privateBucket, false], [publicBucket, true]]) await json('/bucket', 'POST', { id, name: id, public: isPublic })
  for (const bucket of [privateBucket, publicBucket]) await request(`/object/${bucket}/folder/hello.txt`, { method: 'POST', body: content, headers: { 'Content-Type': 'text/plain; charset=utf-8', 'Cache-Control': 'max-age=120' } })
  await json('/object/copy', 'POST', { bucketId: privateBucket, sourceKey: 'folder/hello.txt', destinationKey: 'copy.txt' })
  await json('/object/move', 'POST', { bucketId: privateBucket, sourceKey: 'copy.txt', destinationKey: 'moved.txt' })
  for (const name of ['NUL.txt', 'file:stream', 'trailing.']) await request(`/object/${privateBucket}/${encodeURIComponent(name)}`, { method: 'POST', body: 'blocked', headers: { 'Content-Type': 'text/plain' }, allowed: [400, 403] })
  const sql = `CREATE POLICY native_storage_owner ON storage.objects FOR ALL TO authenticated USING (bucket_id = '${privateBucket}' AND owner_id = auth.uid()::text) WITH CHECK (bucket_id = '${privateBucket}' AND owner_id = auth.uid()::text);`
  const response = await fetch('http://127.0.0.1:54323/api/platform/pg-meta/default/query', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ query: sql }) })
  assert(response.ok, await response.text())
  await request(`/object/${privateBucket}/owned.txt`, { token: user, method: 'POST', body: content, headers: { 'Content-Type': 'text/plain' } })
}
for (const path of [`/object/${privateBucket}/folder/hello.txt`, `/object/${privateBucket}/moved.txt`]) {
  const response = await request(path)
  assert.equal(await response.text(), content)
  assert(response.headers.get('etag'), 'Missing persisted ETag metadata')
  assert.match(response.headers.get('content-type'), /text\/plain/)
  assert.match(response.headers.get('cache-control'), /max-age=120/)
}
assert.equal(await (await request(`/object/${privateBucket}/owned.txt`, { token: user })).text(), content)
await request(`/object/${privateBucket}/folder/hello.txt`, { token: anon, allowed: [400, 403, 404] })
await request(`/object/${privateBucket}/owned.txt`, { token: anon, allowed: [400, 403, 404] })
await request(`/object/${privateBucket}/denied.txt`, { token: anon, method: 'POST', body: 'denied', headers: { 'Content-Type': 'text/plain' }, allowed: [400, 403] })
await request(`/object/${privateBucket}/folder/hello.txt`, { token: user, allowed: [400, 403, 404] })
assert.equal(await (await request(`/object/public/${publicBucket}/folder/hello.txt`, { token: null })).text(), content)
const range = await request(`/object/${privateBucket}/folder/hello.txt`, { headers: { Range: 'bytes=0-5' }, allowed: [206] })
assert.equal(await range.text(), 'Native')
const signed = await (await json(`/object/sign/${privateBucket}/folder/hello.txt`, 'POST', { expiresIn: 60 })).json()
assert.equal(await (await request(signed.signedURL, { token: null })).text(), content)
const objects = await (await json(`/object/list/${privateBucket}`, 'POST', { prefix: 'folder', limit: 100 })).json()
assert(objects.some(item => item.name === 'hello.txt'))
const temp = `delete-${cycle}.txt`
await request(`/object/${privateBucket}/${temp}`, { method: 'POST', body: 'delete me', headers: { 'Content-Type': 'text/plain' } })
await json(`/object/${privateBucket}`, 'DELETE', { prefixes: [temp] })
await request(`/object/${privateBucket}/${temp}`, { allowed: [400, 404] })
console.log(`PASS: Storage cycle ${cycle}: upload, copy/move, download/range, metadata, public/signed URLs, list/delete, RLS and restart persistence.`)
