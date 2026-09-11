import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

class TypeScriptSubmoduleDistribution {
  static platforms = [
    ['arm_darwin', 'tsc'],
    ['x64_linux_gnu', 'tsc'],
    ['x64_window_msvc', 'tsc.exe'],
  ]

  constructor(packageRoot) {
    this.repositoryRoot = resolve(packageRoot, '../..')
    this.submodulePath = 'third_party/typescript'
    this.sourceRoot = join(this.repositoryRoot, this.submodulePath)
    this.packageScript = join(this.sourceRoot, 'tools/scripts/package-arkts.mjs')
  }

  prepare() {
    this.initialize()
    const commit = this.verifySource()
    const distributionRoot = join(this.sourceRoot, 'built/arkts', commit.slice(0, 10))
    if (!existsSync(distributionRoot)) {
      this.build()
    }
    this.verifyDistribution(distributionRoot, commit)
    process.stdout.write(`${distributionRoot}\n`)
  }

  initialize() {
    if (existsSync(this.packageScript)) {
      return
    }
    execFileSync('git', ['submodule', 'update', '--init', '--checkout', '--', this.submodulePath], {
      cwd: this.repositoryRoot,
      stdio: 'inherit',
    })
    if (!existsSync(this.packageScript)) {
      throw new Error(`ARKDOWN_TYPESCRIPT_SUBMODULE_MISSING: ${this.submodulePath} is not initialized`)
    }
  }

  verifySource() {
    const gitlink = this.git(['ls-files', '--stage', '--', this.submodulePath], this.repositoryRoot).match(
      /^160000 ([0-9a-f]{40}) 0\t/,
    )
    if (!gitlink) {
      throw new Error(`ARKDOWN_TYPESCRIPT_GITLINK_MISSING: ${this.submodulePath} is not a registered submodule`)
    }

    const commit = this.git(['rev-parse', 'HEAD'], this.sourceRoot).trim()
    if (commit !== gitlink[1]) {
      throw new Error(`ARKDOWN_TYPESCRIPT_GITLINK_MISMATCH: expected ${gitlink[1]}, received ${commit}`)
    }

    const status = this.git(['status', '--porcelain=v1', '--untracked-files=all'], this.sourceRoot).trim()
    if (status) {
      throw new Error(
        `ARKDOWN_TYPESCRIPT_SUBMODULE_DIRTY: release packaging requires the clean pinned submodule\n${status}`,
      )
    }
    return commit
  }

  build() {
    execFileSync(process.execPath, [this.packageScript], {
      cwd: this.sourceRoot,
      stdio: ['inherit', process.stderr, 'inherit'],
    })
  }

  verifyDistribution(distributionRoot, commit) {
    for (const [directory, executable] of TypeScriptSubmoduleDistribution.platforms) {
      const platformRoot = join(distributionRoot, directory)
      const metadataPath = join(platformRoot, 'BUILD.json')
      const executablePath = join(platformRoot, executable)
      if (!existsSync(metadataPath) || !existsSync(executablePath)) {
        throw new Error(
          `ARKDOWN_TYPESCRIPT_DIST_INCOMPLETE: ${distributionRoot} is missing ${directory}/${executable} or BUILD.json`,
        )
      }

      const metadata = JSON.parse(readFileSync(metadataPath, 'utf8'))
      const digest = createHash('sha256').update(readFileSync(executablePath)).digest('hex')
      if (
        metadata.commit !== commit ||
        metadata.dirty !== false ||
        metadata.embedded_typescript_libs !== true ||
        metadata.binary_sha256 !== digest
      ) {
        throw new Error(`ARKDOWN_TYPESCRIPT_DIST_INVALID: ${platformRoot} does not match clean submodule ${commit}`)
      }
    }
  }

  git(arguments_, cwd) {
    return execFileSync('git', arguments_, { cwd, encoding: 'utf8' })
  }
}

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
new TypeScriptSubmoduleDistribution(packageRoot).prepare()
