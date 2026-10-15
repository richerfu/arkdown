const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { execSync } = require('node:child_process')
const test = require('node:test')
const { PackageLicenseWriter } = require('../scripts/package-licenses.cjs')

const workspaceRoot = path.resolve(__dirname, '../../..')
const sourcePackage = path.resolve(__dirname, '..')

test('npm main and native tarballs carry the original license texts and dual-license metadata', (t) => {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-package-licenses-'))
  t.after(() => fs.rmSync(temporary, { recursive: true, force: true }))
  const packageRoot = path.join(temporary, 'package')
  fs.mkdirSync(packageRoot)
  const mainManifest = JSON.parse(fs.readFileSync(path.join(sourcePackage, 'package.json'), 'utf8'))
  fs.writeFileSync(path.join(packageRoot, 'package.json'), JSON.stringify(mainManifest))
  const targets = ['darwin-arm64', 'linux-x64-gnu', 'win32-x64-msvc']
  for (const target of targets) {
    const directory = path.join(packageRoot, 'npm', target)
    fs.mkdirSync(directory, { recursive: true })
    fs.writeFileSync(
      path.join(directory, 'package.json'),
      JSON.stringify({
        name: `${mainManifest.name}-${target}`,
        version: mainManifest.version,
        license: 'UNLICENSED',
        files: ['arkdown.node'],
      }),
    )
    fs.writeFileSync(path.join(directory, 'arkdown.node'), 'license-packaging-test')
  }
  new PackageLicenseWriter(packageRoot, workspaceRoot).write()
  for (const directory of [packageRoot, ...targets.map((target) => path.join(packageRoot, 'npm', target))]) {
    fs.writeFileSync(path.join(directory, 'licenses/obsolete-notice.txt'), 'removed upstream notice')
  }
  new PackageLicenseWriter(packageRoot, workspaceRoot).write()

  for (const directory of [packageRoot, ...targets.map((target) => path.join(packageRoot, 'npm', target))]) {
    const manifest = JSON.parse(fs.readFileSync(path.join(directory, 'package.json'), 'utf8'))
    assert.equal(manifest.license, 'MIT OR Apache-2.0')
    const result = JSON.parse(
      // npm is a .cmd launcher on Windows. Keep the shell command constant and
      // select the package through cwd so paths never enter shell syntax.
      execSync('npm pack --dry-run --ignore-scripts --offline --json', {
        cwd: directory,
        env: { ...process.env, NPM_CONFIG_CACHE: path.join(temporary, 'npm-cache') },
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'pipe'],
      }),
    )[0]
    const packedFiles = new Set(result.files.map((file) => file.path))
    assert.equal(packedFiles.has('licenses/obsolete-notice.txt'), false, manifest.name)
    for (const name of ['LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE', 'THIRD_PARTY_NOTICES.md']) {
      assert.ok(packedFiles.has(name), `${manifest.name}: ${name}`)
      assert.deepEqual(fs.readFileSync(path.join(directory, name)), fs.readFileSync(path.join(workspaceRoot, name)))
    }
    for (const name of [
      'oxc/LICENSE',
      'oxc/THIRD-PARTY-LICENSE',
      'rolldown/THIRD-PARTY-LICENSE',
      'tsrs/NOTICE',
      'openharmony-ets2bundle/LICENSE',
      'dependencies.json',
      'repositories.json',
    ]) {
      assert.ok(packedFiles.has(`licenses/${name}`), `${manifest.name}: licenses/${name}`)
      assert.deepEqual(
        fs.readFileSync(path.join(directory, 'licenses', name)),
        fs.readFileSync(path.join(workspaceRoot, 'licenses', name)),
      )
    }
    assert.deepEqual(
      fs.readFileSync(path.join(directory, 'tsrs-LICENSE.txt')),
      fs.readFileSync(path.join(workspaceRoot, 'licenses/tsrs/LICENSE')),
    )
    assert.deepEqual(
      fs.readFileSync(path.join(directory, 'tsrs-NOTICE.txt')),
      fs.readFileSync(path.join(workspaceRoot, 'licenses/tsrs/NOTICE')),
    )
  }

  // A distributed npm package can be packed again without a source workspace.
  new PackageLicenseWriter(packageRoot, path.join(temporary, 'no-workspace')).write()
})

test('packaging rejects stale dependency notices after Cargo.lock changes', (t) => {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-stale-licenses-'))
  t.after(() => fs.rmSync(temporary, { recursive: true, force: true }))
  const packageRoot = path.join(temporary, 'packages/arkdown')
  fs.mkdirSync(packageRoot, { recursive: true })
  fs.mkdirSync(path.join(temporary, 'licenses'))
  fs.copyFileSync(path.join(sourcePackage, 'package.json'), path.join(packageRoot, 'package.json'))
  fs.copyFileSync(
    path.join(workspaceRoot, 'licenses/dependencies.json'),
    path.join(temporary, 'licenses/dependencies.json'),
  )
  fs.writeFileSync(path.join(temporary, 'Cargo.toml'), '[workspace]\n')
  fs.writeFileSync(path.join(temporary, 'Cargo.lock'), 'changed dependency graph\n')
  assert.throws(() => new PackageLicenseWriter(packageRoot, temporary).write(), /Cargo.lock changed/)
})
