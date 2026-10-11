import { execFileSync } from 'node:child_process'
import { copyFileSync, existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

// Cargo links the pinned crates into the binding. Release preparation only
// verifies the source and includes the compiler's license and provenance.
export class CompilerSource {
  constructor(repositoryRoot) {
    this.repositoryRoot = repositoryRoot
    this.sourceRoot = join(repositoryRoot, 'third_party/tsrs')
  }

  verify() {
    const gitlink = this.git(['ls-files', '--stage', '--', 'third_party/tsrs'], this.repositoryRoot).match(
      /^160000 ([0-9a-f]{40}) 0\t/,
    )
    if (!gitlink) throw new Error('ARKDOWN_TSRS_GITLINK_MISSING')
    const commit = this.git(['rev-parse', 'HEAD'], this.sourceRoot).trim()
    if (commit !== gitlink[1])
      throw new Error(`ARKDOWN_TSRS_GITLINK_MISMATCH: expected ${gitlink[1]}, received ${commit}`)
    if (this.git(['status', '--porcelain=v1', '--untracked-files=all'], this.sourceRoot).trim())
      throw new Error('ARKDOWN_TSRS_SUBMODULE_DIRTY: release requires the clean pinned compiler crates')
    return commit
  }

  package(platformRoot) {
    const commit = this.verify()
    const platforms = ['darwin-arm64', 'linux-x64-gnu', 'win32-x64-msvc']
    const inputs = platforms.map((platform) => {
      const directory = join(platformRoot, platform)
      return { directory, manifest: JSON.parse(readFileSync(join(directory, 'package.json'), 'utf8')) }
    })
    // Validate every source before changing any package.
    for (const name of ['LICENSE', 'NOTICE']) readFileSync(join(this.sourceRoot, name))
    for (const { directory, manifest } of inputs) {
      const obsolete = /^(?:tsrs(?:\.exe)?|tsc(?:\.exe)?|typescript-.*|tsrs-(?:BUILD\.json|ARKTS\.md))$/
      for (const name of manifest.files ?? []) if (obsolete.test(name)) rmSync(join(directory, name), { force: true })
      for (const name of ['tsrs', 'tsrs.exe', 'tsc', 'tsc.exe']) rmSync(join(directory, name), { force: true })
      for (const name of ['LICENSE', 'NOTICE'])
        copyFileSync(join(this.sourceRoot, name), join(directory, `tsrs-${name}.txt`))
      writeFileSync(
        join(directory, 'tsrs-SOURCE.json'),
        `${JSON.stringify(
          {
            repository: 'https://github.com/ohos-rs/tsrs',
            commit,
            integration: 'rust-crate',
            embedded_typescript_libs: true,
          },
          null,
          2,
        )}\n`,
      )
      manifest.files = [
        ...new Set([
          ...(manifest.files ?? []).filter((name) => !obsolete.test(name)),
          'tsrs-LICENSE.txt',
          'tsrs-NOTICE.txt',
          'tsrs-SOURCE.json',
        ]),
      ]
      writeFileSync(join(directory, 'package.json'), `${JSON.stringify(manifest, null, 2)}\n`)
    }
  }

  git(args, cwd) {
    return execFileSync('git', args, { cwd, encoding: 'utf8' })
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const source = new CompilerSource(resolve(dirname(fileURLToPath(import.meta.url)), '../../..'))
  const args = process.argv.slice(2)
  if (args.length === 1 && args[0] === '--verify') source.verify()
  else if (args.length === 2 && args[0] === '--package' && existsSync(args[1])) source.package(resolve(args[1]))
  else throw new Error('Usage: node compiler-source.mjs --verify | --package <npm-platform-root>')
}
