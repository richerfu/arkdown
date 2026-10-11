'use strict'
const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
const { createHash } = require('node:crypto')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')

const platforms = [
  ['aarch64-apple-darwin', 'darwin-arm64', 'tsrs'],
  ['x86_64-unknown-linux-gnu', 'linux-x64-gnu', 'tsrs'],
  ['x86_64-pc-windows-msvc', 'win32-x64-msvc', 'tsrs.exe'],
]

test('packages tsrs on all platforms, removes tsgo and rejects mixed or corrupt inputs before writing', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-tsrs-dist-'))
  try {
    const dist = path.join(root, 'dist')
    const npm = path.join(root, 'npm')
    for (const [target, platform, binary] of platforms) {
      const source = path.join(dist, target)
      const output = path.join(npm, platform)
      fs.mkdirSync(source, { recursive: true })
      fs.mkdirSync(output, { recursive: true })
      const content = Buffer.from(`binary fixture for ${target}`)
      fs.writeFileSync(path.join(source, binary), content)
      fs.writeFileSync(
        path.join(source, 'BUILD.json'),
        JSON.stringify({
          commit: 'a'.repeat(40),
          dirty: false,
          target,
          api_protocol: 10,
          embedded_typescript_libs: true,
          binary_sha256: createHash('sha256').update(content).digest('hex'),
        }),
      )
      for (const file of ['ARKTS.md', 'LICENSE.txt', 'NOTICE.txt']) fs.writeFileSync(path.join(source, file), file)
      fs.writeFileSync(path.join(output, 'tsc'), 'old compiler')
      fs.writeFileSync(path.join(output, 'typescript-BUILD.json'), '{}')
      fs.writeFileSync(
        path.join(output, 'package.json'),
        JSON.stringify({ files: ['binding.node', 'tsc', 'typescript-BUILD.json'] }),
      )
    }
    const script = path.resolve(__dirname, '../scripts/attach-tsrs-dist.mjs')
    const run = () => spawnSync(process.execPath, [script, dist, npm], { encoding: 'utf8' })
    const last = path.join(dist, platforms[2][0], 'BUILD.json')
    const metadata = JSON.parse(fs.readFileSync(last, 'utf8'))
    fs.writeFileSync(last, JSON.stringify({ ...metadata, commit: 'b'.repeat(40) }))
    assert.match(run().stderr, /MIXED_COMMITS/)
    assert.equal(fs.existsSync(path.join(npm, platforms[0][1], 'tsrs')), false)
    fs.writeFileSync(last, JSON.stringify({ ...metadata, binary_sha256: '0'.repeat(64) }))
    assert.match(run().stderr, /HASH_MISMATCH/)
    assert.equal(fs.existsSync(path.join(npm, platforms[0][1], 'tsrs')), false)
    fs.writeFileSync(last, JSON.stringify(metadata))
    const result = run()
    assert.equal(result.status, 0, result.stderr)
    for (const [target, platform, binary] of platforms) {
      const output = path.join(npm, platform)
      assert.deepEqual(fs.readFileSync(path.join(output, binary)), fs.readFileSync(path.join(dist, target, binary)))
      assert.equal(fs.existsSync(path.join(output, 'tsc')), false)
      assert.equal(fs.existsSync(path.join(output, 'typescript-BUILD.json')), false)
      const files = JSON.parse(fs.readFileSync(path.join(output, 'package.json'), 'utf8')).files
      assert.ok(files.includes(binary) && files.includes('tsrs-LICENSE.txt') && files.includes('binding.node'))
      assert.ok(!files.includes('tsc') && !files.includes('typescript-BUILD.json'))
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true })
  }
})
