import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

class TsrsSubmoduleDistribution {
  static targets = ['aarch64-apple-darwin', 'x86_64-unknown-linux-gnu', 'x86_64-pc-windows-msvc']

  constructor(packageRoot, targets) {
    this.repositoryRoot = resolve(packageRoot, '../..')
    this.submodulePath = 'third_party/tsrs'
    this.sourceRoot = join(this.repositoryRoot, this.submodulePath)
    this.targets = targets
  }

  prepare() {
    this.initialize()
    const commit = this.verifySource()
    const distributionRoot = join(this.sourceRoot, 'target/arkdown-dist', commit)
    for (const target of this.targets) {
      if (!TsrsSubmoduleDistribution.targets.includes(target)) throw new Error(`Unsupported tsrs target: ${target}`)
      const platformRoot = join(distributionRoot, target)
      if (!existsSync(join(platformRoot, 'BUILD.json'))) this.build(target, platformRoot, commit)
      this.verifyDistribution(platformRoot, target, commit)
    }
    // stdout is consumed by the release scripts; Cargo output goes to stderr.
    process.stdout.write(`${distributionRoot}\n`)
  }

  initialize() {
    if (existsSync(join(this.sourceRoot, 'Cargo.toml'))) return
    execFileSync('git', ['submodule', 'update', '--init', '--checkout', '--', this.submodulePath], {
      cwd: this.repositoryRoot,
      stdio: ['inherit', process.stderr, 'inherit'],
    })
  }

  verifySource() {
    const gitlink = this.git(['ls-files', '--stage', '--', this.submodulePath], this.repositoryRoot).match(
      /^160000 ([0-9a-f]{40}) 0\t/,
    )
    if (!gitlink) throw new Error(`ARKDOWN_TSRS_GITLINK_MISSING: ${this.submodulePath}`)
    const commit = this.git(['rev-parse', 'HEAD'], this.sourceRoot).trim()
    if (commit !== gitlink[1])
      throw new Error(`ARKDOWN_TSRS_GITLINK_MISMATCH: expected ${gitlink[1]}, received ${commit}`)
    const status = this.git(['status', '--porcelain=v1', '--untracked-files=all'], this.sourceRoot).trim()
    if (status)
      throw new Error(`ARKDOWN_TSRS_SUBMODULE_DIRTY: release packaging requires the clean pinned submodule\n${status}`)
    return commit
  }

  build(target, platformRoot, commit) {
    const run = (command, args) =>
      execFileSync(command, args, {
        cwd: this.sourceRoot,
        env: { ...process.env, CARGO_TARGET_DIR: join(this.sourceRoot, 'target') },
        stdio: ['inherit', process.stderr, 'inherit'],
      })
    run('rustup', ['target', 'add', target])
    const native =
      (target.includes('apple-darwin') && process.platform === 'darwin') ||
      (target.includes('linux-gnu') && process.platform === 'linux') ||
      (target.includes('windows-msvc') && process.platform === 'win32')
    const command = native ? ['build'] : target.includes('windows-msvc') ? ['xwin', 'build'] : ['zigbuild']
    run('cargo', [...command, '--release', '--locked', '--target', target, '-p', 'tsrs_cli'])
    const executable = target.includes('windows') ? 'tsrs.exe' : 'tsrs'
    mkdirSync(platformRoot, { recursive: true })
    const output = join(platformRoot, executable)
    copyFileSync(join(this.sourceRoot, 'target', target, 'release', executable), output)
    if (executable === 'tsrs') chmodSync(output, 0o755)
    for (const [source, destination] of [
      ['LICENSE', 'LICENSE.txt'],
      ['NOTICE', 'NOTICE.txt'],
      ['docs/ARKTS.md', 'ARKTS.md'],
    ]) {
      copyFileSync(join(this.sourceRoot, source), join(platformRoot, destination))
    }
    const metadata = {
      commit,
      dirty: false,
      target,
      api_protocol: 10,
      embedded_typescript_libs: true,
      rustc: execFileSync('rustc', ['--version'], { cwd: this.sourceRoot, encoding: 'utf8' }).trim(),
      binary_sha256: createHash('sha256').update(readFileSync(output)).digest('hex'),
    }
    writeFileSync(join(platformRoot, 'BUILD.json'), `${JSON.stringify(metadata, null, 2)}\n`)
  }

  verifyDistribution(platformRoot, target, commit) {
    const executable = join(platformRoot, target.includes('windows') ? 'tsrs.exe' : 'tsrs')
    const metadata = JSON.parse(readFileSync(join(platformRoot, 'BUILD.json'), 'utf8'))
    const digest = createHash('sha256').update(readFileSync(executable)).digest('hex')
    if (
      metadata.commit !== commit ||
      metadata.dirty !== false ||
      metadata.target !== target ||
      metadata.api_protocol !== 10 ||
      metadata.embedded_typescript_libs !== true ||
      metadata.binary_sha256 !== digest
    ) {
      throw new Error(`ARKDOWN_TSRS_DIST_INVALID: ${platformRoot} does not match clean submodule ${commit}`)
    }
    for (const file of ['LICENSE.txt', 'NOTICE.txt', 'ARKTS.md']) {
      if (!existsSync(join(platformRoot, file)))
        throw new Error(`ARKDOWN_TSRS_DIST_INCOMPLETE: ${platformRoot}/${file}`)
    }
  }

  git(args, cwd) {
    return execFileSync('git', args, { cwd, encoding: 'utf8' })
  }
}

const args = process.argv.slice(2)
if (args.length && (args.length !== 2 || args[0] !== '--target')) {
  throw new Error('Usage: node build-tsrs-dist.mjs [--target <Rust target triple>]')
}
const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
new TsrsSubmoduleDistribution(packageRoot, args.length ? [args[1]] : TsrsSubmoduleDistribution.targets).prepare()
