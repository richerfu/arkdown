import { createHash } from 'node:crypto'
import { copyFileSync, readFileSync, chmodSync, writeFileSync, rmSync } from 'node:fs'
import { basename, join } from 'node:path'

class NativeTsrsDistribution {
  static platforms = [
    {
      sourceDirectory: 'aarch64-apple-darwin',
      sourceExecutable: 'tsrs',
      packageDirectory: 'darwin-arm64',
      packageExecutable: 'tsrs',
    },
    {
      sourceDirectory: 'x86_64-unknown-linux-gnu',
      sourceExecutable: 'tsrs',
      packageDirectory: 'linux-x64-gnu',
      packageExecutable: 'tsrs',
    },
    {
      sourceDirectory: 'x86_64-pc-windows-msvc',
      sourceExecutable: 'tsrs.exe',
      packageDirectory: 'win32-x64-msvc',
      packageExecutable: 'tsrs.exe',
    },
  ]

  constructor(distributionRoot, npmRoot) {
    this.distributionRoot = distributionRoot
    this.npmRoot = npmRoot
  }

  attach() {
    // Validate the complete set before changing any package.
    const builds = NativeTsrsDistribution.platforms.map((platform) => this.verifyPlatform(platform))
    const commits = new Set(builds.map((build) => build.commit))
    if (commits.size !== 1) {
      throw new Error(`ARKDOWN_TSRS_DIST_MIXED_COMMITS: ${[...commits].join(', ')}`)
    }
    for (const platform of NativeTsrsDistribution.platforms) this.attachPlatform(platform)
    process.stdout.write(`Attached native ArkTS tsrs ${[...commits][0]} to three platform packages.\n`)
  }

  verifyPlatform(platform) {
    const sourceRoot = join(this.distributionRoot, platform.sourceDirectory)
    const build = JSON.parse(readFileSync(join(sourceRoot, 'BUILD.json'), 'utf8'))
    if (
      build.dirty !== false ||
      build.embedded_typescript_libs !== true ||
      build.api_protocol !== 10 ||
      build.target !== platform.sourceDirectory ||
      !/^[0-9a-f]{40}$/.test(build.commit)
    ) {
      throw new Error(
        `ARKDOWN_TSRS_DIST_UNRELEASABLE: ${sourceRoot}/BUILD.json must describe a clean build with embedded TypeScript libraries`,
      )
    }

    const sourceExecutable = join(sourceRoot, platform.sourceExecutable)
    const digest = createHash('sha256').update(readFileSync(sourceExecutable)).digest('hex')
    if (digest !== build.binary_sha256) {
      throw new Error(
        `ARKDOWN_TSRS_DIST_HASH_MISMATCH: ${sourceExecutable} expected ${build.binary_sha256}, received ${digest}`,
      )
    }

    for (const name of ['ARKTS.md', 'LICENSE.txt', 'NOTICE.txt']) readFileSync(join(sourceRoot, name))
    return build
  }

  attachPlatform(platform) {
    const sourceRoot = join(this.distributionRoot, platform.sourceDirectory)
    const packageRoot = join(this.npmRoot, platform.packageDirectory)
    const sourceExecutable = join(sourceRoot, platform.sourceExecutable)
    const obsolete = [
      'tsc',
      'tsc.exe',
      'typescript-ARKTS.md',
      'typescript-BUILD.json',
      'typescript-LICENSE.txt',
      'typescript-NOTICE.txt',
    ]
    for (const name of obsolete) rmSync(join(packageRoot, name), { force: true })
    const packageExecutable = join(packageRoot, platform.packageExecutable)
    copyFileSync(sourceExecutable, packageExecutable)
    if (platform.packageExecutable === 'tsrs') {
      chmodSync(packageExecutable, 0o755)
    }

    const copiedMetadata = [
      ['ARKTS.md', 'tsrs-ARKTS.md'],
      ['BUILD.json', 'tsrs-BUILD.json'],
      ['LICENSE.txt', 'tsrs-LICENSE.txt'],
      ['NOTICE.txt', 'tsrs-NOTICE.txt'],
    ]
    for (const [source, destination] of copiedMetadata) {
      copyFileSync(join(sourceRoot, source), join(packageRoot, destination))
    }

    const packageJsonPath = join(packageRoot, 'package.json')
    const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf8'))
    const files = [
      ...(packageJson.files ?? []).filter((name) => !obsolete.includes(name)),
      platform.packageExecutable,
      ...copiedMetadata.map(([, destination]) => destination),
    ]
    packageJson.files = [...new Set(files)].sort((left, right) => basename(left).localeCompare(basename(right)))
    writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`)
  }
}

const [distributionRoot, npmRoot] = process.argv.slice(2)
if (!distributionRoot || !npmRoot) {
  throw new Error('Usage: node attach-tsrs-dist.mjs <tsrs-dist-root> <npm-platform-root>')
}

new NativeTsrsDistribution(distributionRoot, npmRoot).attach()
