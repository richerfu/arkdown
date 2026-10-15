const fs = require('node:fs')
const path = require('node:path')
const crypto = require('node:crypto')

class PackageLicenseWriter {
  static files = [
    'LICENSE',
    'LICENSE-MIT',
    'LICENSE-APACHE',
    'NOTICE',
    'THIRD_PARTY_NOTICES.md',
    'licenses',
    'tsrs-LICENSE.txt',
    'tsrs-NOTICE.txt',
  ]

  constructor(packageRoot, workspaceRoot = path.resolve(packageRoot, '..', '..')) {
    this.packageRoot = packageRoot
    this.workspaceRoot = workspaceRoot
  }

  write() {
    const manifest = JSON.parse(fs.readFileSync(path.join(this.packageRoot, 'package.json'), 'utf8'))
    if (manifest.license !== 'MIT OR Apache-2.0') {
      throw new Error('arkdown package metadata must declare MIT OR Apache-2.0')
    }

    // In a source checkout the workspace owns the canonical license bundle.
    // An already packed npm package carries the complete bundle itself.
    if (fs.existsSync(path.join(this.workspaceRoot, 'Cargo.toml'))) {
      const inventory = JSON.parse(fs.readFileSync(path.join(this.workspaceRoot, 'licenses/dependencies.json'), 'utf8'))
      const lockHash = crypto
        .createHash('sha256')
        .update(fs.readFileSync(path.join(this.workspaceRoot, 'Cargo.lock')))
        .digest('hex')
      if (inventory.cargo_lock_sha256 !== lockHash) {
        throw new Error(
          'Cargo.lock changed: update licenses/dependencies.json and the upstream license texts before packaging',
        )
      }
      for (const name of PackageLicenseWriter.files.slice(0, 6)) {
        this.copy(path.join(this.workspaceRoot, name), path.join(this.packageRoot, name))
      }
      for (const [source, destination] of [
        ['LICENSE', 'tsrs-LICENSE.txt'],
        ['NOTICE', 'tsrs-NOTICE.txt'],
      ]) {
        fs.copyFileSync(
          path.join(this.workspaceRoot, 'licenses', 'tsrs', source),
          path.join(this.packageRoot, destination),
        )
      }
    }

    for (const name of PackageLicenseWriter.files) {
      const location = path.join(this.packageRoot, name)
      if (!fs.existsSync(location) || (fs.statSync(location).isFile() && fs.statSync(location).size === 0)) {
        throw new Error(`Missing package license material: ${name}`)
      }
    }
    const repositories = JSON.parse(fs.readFileSync(path.join(this.packageRoot, 'licenses/repositories.json'), 'utf8'))
    const inventory = JSON.parse(fs.readFileSync(path.join(this.packageRoot, 'licenses/dependencies.json'), 'utf8'))
    const verified = new Set()
    for (const record of [...repositories, ...inventory.packages]) {
      for (const file of record.files) {
        const identity = `${file.path}:${file.sha256}`
        if (verified.has(identity)) continue
        const content = fs.readFileSync(path.join(this.packageRoot, 'licenses', file.path))
        if (content.length === 0 || crypto.createHash('sha256').update(content).digest('hex') !== file.sha256) {
          throw new Error(`Missing or modified upstream license text: ${file.path}`)
        }
        verified.add(identity)
      }
    }

    const npmRoot = path.join(this.packageRoot, 'npm')
    if (!fs.existsSync(npmRoot)) return
    for (const entry of fs.readdirSync(npmRoot, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue
      const directory = path.join(npmRoot, entry.name)
      const manifestPath = path.join(directory, 'package.json')
      if (!fs.existsSync(manifestPath)) continue
      const platformManifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'))
      if (!platformManifest.name.startsWith(`${manifest.name}-`)) {
        throw new Error(`Unexpected native package: ${platformManifest.name}`)
      }
      for (const name of PackageLicenseWriter.files) {
        this.copy(path.join(this.packageRoot, name), path.join(directory, name))
      }
      platformManifest.license = manifest.license
      platformManifest.files = [...new Set([...(platformManifest.files ?? []), ...PackageLicenseWriter.files])]
      fs.writeFileSync(manifestPath, `${JSON.stringify(platformManifest, null, 2)}\n`)
    }
  }

  copy(source, destination) {
    if (fs.statSync(source).isDirectory()) {
      fs.rmSync(destination, { recursive: true, force: true })
    }
    fs.cpSync(source, destination, { recursive: true })
  }
}

module.exports = { PackageLicenseWriter }

if (require.main === module) {
  new PackageLicenseWriter(path.resolve(__dirname, '..')).write()
}
