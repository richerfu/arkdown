import { createHash } from 'node:crypto'
import { copyFileSync, readFileSync, chmodSync, writeFileSync } from 'node:fs'
import { basename, join } from 'node:path'

class NativeTypeScriptDistribution {
  static platforms = [
    {
      sourceDirectory: 'arm_darwin',
      sourceExecutable: 'tsc',
      packageDirectory: 'darwin-arm64',
      packageExecutable: 'tsc',
    },
    {
      sourceDirectory: 'x64_linux_gnu',
      sourceExecutable: 'tsc',
      packageDirectory: 'linux-x64-gnu',
      packageExecutable: 'tsc',
    },
    {
      sourceDirectory: 'x64_window_msvc',
      sourceExecutable: 'tsc.exe',
      packageDirectory: 'win32-x64-msvc',
      packageExecutable: 'tsc.exe',
    },
  ]

  constructor(distributionRoot, npmRoot) {
    this.distributionRoot = distributionRoot
    this.npmRoot = npmRoot
  }

  attach() {
    const commits = new Set()
    for (const platform of NativeTypeScriptDistribution.platforms) {
      commits.add(this.attachPlatform(platform))
    }
    if (commits.size !== 1) {
      throw new Error(
        `ARKDOWN_TYPESCRIPT_DIST_MIXED_COMMITS: ${[...commits].join(', ')}`,
      )
    }
    process.stdout.write(
      `Attached native ArkTS TypeScript ${[...commits][0]} to three platform packages.\n`,
    )
  }

  attachPlatform(platform) {
    const sourceRoot = join(this.distributionRoot, platform.sourceDirectory)
    const packageRoot = join(this.npmRoot, platform.packageDirectory)
    const build = JSON.parse(readFileSync(join(sourceRoot, 'BUILD.json'), 'utf8'))
    if (build.dirty !== false || build.embedded_typescript_libs !== true) {
      throw new Error(
        `ARKDOWN_TYPESCRIPT_DIST_UNRELEASABLE: ${sourceRoot}/BUILD.json must describe a clean build with embedded TypeScript libraries`,
      )
    }

    const sourceExecutable = join(sourceRoot, platform.sourceExecutable)
    const digest = createHash('sha256')
      .update(readFileSync(sourceExecutable))
      .digest('hex')
    if (digest !== build.binary_sha256) {
      throw new Error(
        `ARKDOWN_TYPESCRIPT_DIST_HASH_MISMATCH: ${sourceExecutable} expected ${build.binary_sha256}, received ${digest}`,
      )
    }

    const packageExecutable = join(packageRoot, platform.packageExecutable)
    copyFileSync(sourceExecutable, packageExecutable)
    if (platform.packageExecutable === 'tsc') {
      chmodSync(packageExecutable, 0o755)
    }

    const copiedMetadata = [
      ['ARKTS.md', 'typescript-ARKTS.md'],
      ['BUILD.json', 'typescript-BUILD.json'],
      ['LICENSE.txt', 'typescript-LICENSE.txt'],
      ['NOTICE.txt', 'typescript-NOTICE.txt'],
    ]
    for (const [source, destination] of copiedMetadata) {
      copyFileSync(join(sourceRoot, source), join(packageRoot, destination))
    }

    const packageJsonPath = join(packageRoot, 'package.json')
    const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf8'))
    const files = [
      ...(packageJson.files ?? []),
      platform.packageExecutable,
      ...copiedMetadata.map(([, destination]) => destination),
    ]
    packageJson.files = [...new Set(files)].sort((left, right) =>
      basename(left).localeCompare(basename(right)),
    )
    writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`)
    return build.commit
  }
}

const [distributionRoot, npmRoot] = process.argv.slice(2)
if (!distributionRoot || !npmRoot) {
  throw new Error(
    'Usage: node attach-typescript-dist.mjs <typescript-dist-root> <npm-platform-root>',
  )
}

new NativeTypeScriptDistribution(distributionRoot, npmRoot).attach()
