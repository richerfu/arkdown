const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { execFileSync } = require('node:child_process')
const { test } = require('node:test')

test('crate packaging includes provenance and notices, rejects dirty pins, and ships no compiler executable', async (context) => {
  const { CompilerSource } = await import('../scripts/compiler-source.mjs')
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-compiler-source-'))
  context.after(() => fs.rmSync(root, { recursive: true, force: true }))
  const compiler = path.join(root, 'third_party/tsrs')
  fs.mkdirSync(compiler, { recursive: true })
  const git = (cwd, ...args) =>
    execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim()
  for (const cwd of [root, compiler]) {
    git(cwd, 'init', '-q')
    git(cwd, 'config', 'user.name', 'Fixture')
    git(cwd, 'config', 'user.email', 'fixture@example.invalid')
    git(cwd, 'config', 'commit.gpgsign', 'false')
  }
  for (const file of ['LICENSE', 'NOTICE']) fs.writeFileSync(path.join(compiler, file), file)
  git(compiler, 'add', '.')
  git(compiler, 'commit', '-qm', 'fixture')
  const commit = git(compiler, 'rev-parse', 'HEAD')
  git(root, 'update-index', '--add', '--cacheinfo', `160000,${commit},third_party/tsrs`)
  const platforms = ['darwin-arm64', 'linux-x64-gnu', 'win32-x64-msvc']
  for (const platform of platforms) {
    const dir = path.join(root, 'npm', platform)
    fs.mkdirSync(dir, { recursive: true })
    const files = ['binding.node', 'tsrs', 'tsrs.exe', 'tsc', 'tsc.exe', 'typescript-BUILD.json', 'tsrs-BUILD.json']
    fs.writeFileSync(path.join(dir, 'package.json'), JSON.stringify({ name: platform, files }))
    for (const file of files) fs.writeFileSync(path.join(dir, file), 'fixture')
  }
  const source = new CompilerSource(root)
  fs.writeFileSync(path.join(compiler, 'NOTICE'), 'dirty')
  assert.throws(() => source.package(path.join(root, 'npm')), /SUBMODULE_DIRTY/)
  assert.equal(fs.existsSync(path.join(root, 'npm', platforms[0], 'tsrs-SOURCE.json')), false)
  fs.writeFileSync(path.join(compiler, 'NOTICE'), 'NOTICE')
  source.package(path.join(root, 'npm'))
  for (const platform of platforms) {
    const dir = path.join(root, 'npm', platform)
    const manifest = JSON.parse(fs.readFileSync(path.join(dir, 'package.json')))
    assert.deepEqual(manifest.files, ['binding.node', 'tsrs-LICENSE.txt', 'tsrs-NOTICE.txt', 'tsrs-SOURCE.json'])
    assert.equal(JSON.parse(fs.readFileSync(path.join(dir, 'tsrs-SOURCE.json'))).commit, commit)
    for (const removed of ['tsrs', 'tsrs.exe', 'tsc', 'tsc.exe', 'typescript-BUILD.json', 'tsrs-BUILD.json']) {
      assert.equal(fs.existsSync(path.join(dir, removed)), false)
    }
  }
  git(root, 'update-index', '--cacheinfo', `160000,${'a'.repeat(40)},third_party/tsrs`)
  assert.throws(() => source.verify(), /GITLINK_MISMATCH/)
})
