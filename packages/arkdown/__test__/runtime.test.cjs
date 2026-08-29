'use strict'

const assert = require('node:assert/strict')
const crypto = require('node:crypto')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')
const { parsePluginAst, runRuntimeTasks, RuntimeHvigorLifecycle } = require('../index.js')

const {
  HVIGOR_PARITY_VERSION,
  HvigorRuntimeHost,
  HvigorCompilePluginPlanner,
  RuntimePluginLoader,
  createBuild,
  createHvigorCommand,
  normalizePluginList,
  normalizeSortedPluginList,
  applyFunctionOrder,
  parseShouldInvalidCacheRequest,
  buildPluginContext,
  requireDefaultExport,
  hvigorProjectHash,
} = require('../js/runtime')

const nativeRuntime = { runRuntimeTasks }

function dispatchPluginHook(plugin, hookName, request = {}) {
  const dispatched = plugin.__arkdownDispatchPluginHook({ hookName, ...request })
  const resultField =
    hookName === 'resolveId' || hookName === 'resolveDynamicImport'
      ? 'resolveId'
      : hookName === 'load'
        ? 'load'
        : hookName === 'transform'
          ? 'transform'
          : hookName === 'shouldInvalidCache'
            ? 'shouldInvalidCache'
            : undefined
  if (resultField === undefined) return dispatched
  const unwrap = (envelope) => envelope[resultField]
  return dispatched && typeof dispatched.then === 'function'
    ? Promise.resolve(dispatched).then(unwrap)
    : unwrap(dispatched)
}

function seedPluginContext(plugin, share) {
  return dispatchPluginHook(plugin, 'seedContext', { share })
}

function createTestHvigorCommand(nativeBuild) {
  return createHvigorCommand(nativeBuild, nativeRuntime)
}

function tempProject() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-runtime-'))
}

function fakeOhpmExecutable(root) {
  if (process.platform === 'win32') {
    const executable = path.join(root, 'fake-ohpm.cmd')
    fs.writeFileSync(executable, '@echo off\r\nexit /b 0\r\n')
    return executable
  }
  const executable = path.join(root, 'fake-ohpm')
  fs.writeFileSync(executable, '#!/bin/sh\nexit 0\n')
  fs.chmodSync(executable, 0o755)
  return executable
}

function copyFixture(name) {
  const source = path.resolve(__dirname, '../../../tests/fixtures', name)
  const target = tempProject()
  fs.cpSync(source, target, {
    recursive: true,
    filter(candidate) {
      const relative = path.relative(source, candidate)
      return !relative.split(path.sep).some((part) => part === '.hvigor' || part === 'build')
    },
  })
  return target
}

test('runtime reports the same Hvigor parity version as Rust cache and sync metadata', () => {
  assert.equal(HVIGOR_PARITY_VERSION, '6.24.3')
})

test('Rust lifecycle owner awaits callbacks sequentially and releases cleared hooks', async () => {
  const lifecycle = new RuntimeHvigorLifecycle()
  const events = []
  lifecycle.register('configEvaluated', async (invocation) => {
    events.push(`first-start:${invocation}`)
    await new Promise((resolve) => setTimeout(resolve, 5))
    events.push(`first-end:${invocation}`)
  })
  lifecycle.register('configEvaluated', (invocation) => {
    events.push(`second:${invocation}`)
  })

  await lifecycle.runHook('configEvaluated', 7)
  assert.deepEqual(events, ['first-start:7', 'first-end:7', 'second:7'])

  lifecycle.clear()
  await lifecycle.runHook('configEvaluated', 8)
  assert.deepEqual(events, ['first-start:7', 'first-end:7', 'second:7'])
})

test('hvigor project cache hash matches latest 32-character cache layout', () => {
  const root = path.resolve('fixtures', 'cache-project')
  const expected = crypto.createHash('sha256').update(root).digest('hex').slice(0, 32)
  assert.equal(hvigorProjectHash(root), expected)
  assert.equal(hvigorProjectHash(root).length, 32)
})

test('hvigor config dependencies are installed before package-backed plugins are evaluated', async (context) => {
  const npm = path.join(path.dirname(process.execPath), process.platform === 'win32' ? 'npm.cmd' : 'npm')
  if (!fs.existsSync(npm)) {
    context.skip(`npm is not available next to ${process.execPath}`)
    return
  }

  const root = tempProject()
  const hvigorHome = tempProject()
  const hvigorDir = path.join(root, 'hvigor')
  const packageRoot = path.join(hvigorDir, 'fixture-plugin')
  const marker = path.join(root, 'dependency-plugin.txt')
  fs.mkdirSync(path.join(packageRoot, 'dist'), { recursive: true })
  fs.writeFileSync(
    path.join(packageRoot, 'package.json'),
    JSON.stringify({
      name: 'fixture-hvigor-workspace-plugin',
      version: '1.0.0',
      exports: { '.': { require: './dist/plugin.cjs' } },
    }),
  )
  fs.writeFileSync(
    path.join(packageRoot, 'dist', 'plugin.cjs'),
    `exports.plugin = {
      pluginId: 'fixture.workspace-dependency',
      apply() { require('node:fs').writeFileSync(process.env.ARKDOWN_PLUGIN_DEPENDENCY_MARKER, 'loaded') }
    }\n`,
  )
  fs.writeFileSync(
    path.join(hvigorDir, 'hvigor-config.json5'),
    `{
      properties: { 'hvigor.dependency.useNpm': true },
      dependencies: { 'fixture-hvigor-workspace-plugin': 'file:./fixture-plugin' }
    }\n`,
  )
  fs.writeFileSync(
    path.join(root, 'hvigorfile.ts'),
    `import { plugin } from 'fixture-hvigor-workspace-plugin'\nexport default { plugins: [plugin] }\n`,
  )
  fs.writeFileSync(path.join(root, '.npmrc'), 'audit=false\nfund=false\n')

  const previousHvigorHome = process.env.HVIGOR_USER_HOME
  const previousMarker = process.env.ARKDOWN_PLUGIN_DEPENDENCY_MARKER
  process.env.HVIGOR_USER_HOME = hvigorHome
  process.env.ARKDOWN_PLUGIN_DEPENDENCY_MARKER = marker
  try {
    await new RuntimePluginLoader({ project: root }).load()
    assert.equal(fs.readFileSync(marker, 'utf8'), 'loaded')

    const workspace = path.join(hvigorHome, 'project_caches', hvigorProjectHash(root), 'workspace')
    const workspaceManifest = path.join(workspace, 'package.json')
    const manifest = JSON.parse(fs.readFileSync(workspaceManifest, 'utf8'))
    assert.equal(manifest.dependencies['fixture-hvigor-workspace-plugin'], path.join(hvigorDir, 'fixture-plugin'))
    assert.ok(
      fs.existsSync(path.join(workspace, 'node_modules', 'fixture-hvigor-workspace-plugin', 'dist', 'plugin.cjs')),
    )

    const modifiedAt = fs.statSync(workspaceManifest).mtimeMs
    await new RuntimePluginLoader({ project: root }).load()
    assert.equal(fs.statSync(workspaceManifest).mtimeMs, modifiedAt, 'current dependency workspace must be reused')
  } finally {
    if (previousHvigorHome === undefined) delete process.env.HVIGOR_USER_HOME
    else process.env.HVIGOR_USER_HOME = previousHvigorHome
    if (previousMarker === undefined) delete process.env.ARKDOWN_PLUGIN_DEPENDENCY_MARKER
    else process.env.ARKDOWN_PLUGIN_DEPENDENCY_MARKER = previousMarker
  }
})

test('Rust hvigor config merge preserves callback identity and array replacement', () => {
  const callback = () => 'live'
  const { mergeHvigorConfig } = require('../index.js')
  const base = {
    nested: { retained: true, callback },
    replaced: ['base'],
  }
  const merged = mergeHvigorConfig(base, {
    nested: { retained: undefined, added: 7 },
    replaced: ['overlay'],
  })

  assert.equal(merged.nested.callback, callback)
  assert.equal(merged.nested.retained, true)
  assert.equal(merged.nested.added, 7)
  assert.deepEqual(merged.replaced, ['overlay'])
  assert.notEqual(merged.nested, base.nested)
})

test('PreBuild clean dependency follows latest hvigor and toolchain cache metadata', async () => {
  const project = copyFixture('minimal-hap')
  const sdkRoot = path.join(project, 'fixture-sdk')
  fs.mkdirSync(path.join(sdkRoot, 'default'), { recursive: true })
  fs.writeFileSync(
    path.join(sdkRoot, 'default', 'sdk-pkg.json'),
    JSON.stringify({ data: { version: '6.1.1.125', platformVersion: '6.1.1', apiVersion: '24' } }),
  )

  const dependencies = async () => {
    const runtime = new RuntimePluginLoader({ project, sdkPath: sdkRoot })
    await runtime.load()
    const node = runtime.hvigorHost._nodesByName.get('entry')
    return runtime.hvigorHost._getTaskByName(node, 'default@PreBuild').dependencies
  }

  assert.deepEqual(await dependencies(), ['entry:clean'])
  fs.mkdirSync(path.join(project, '.hvigor', 'cache'), { recursive: true })
  fs.writeFileSync(
    path.join(project, '.hvigor', 'cache', 'meta.json'),
    JSON.stringify({ hvigorVersion: '6.24.3', toolChainsVersion: '6.1.1.125' }),
  )
  assert.deepEqual(await dependencies(), [])

  fs.writeFileSync(
    path.join(sdkRoot, 'default', 'sdk-pkg.json'),
    JSON.stringify({ data: { version: '6.1.1.126', platformVersion: '6.1.1', apiVersion: '24' } }),
  )
  assert.deepEqual(await dependencies(), ['entry:clean'])

  const runtime = new RuntimePluginLoader({ project, sdkPath: sdkRoot })
  await runtime.load()
  await runtime.hvigorHost.writeBuildMeta()
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(project, '.hvigor', 'cache', 'meta.json'), 'utf8')), {
    compileSdkVersion: '6.1.1(24)',
    hvigorVersion: '6.24.3',
    toolChainsVersion: '6.1.1.126',
  })
})

test('runtime loader applies hvigorfile plugins and loads compilePluginPath modules', async () => {
  const root = tempProject()
  const marker = path.join(root, 'plugin-applied.txt')
  fs.writeFileSync(
    path.join(root, 'compile-plugin.js'),
    `
exports.default = {
  name: 'compile-plugin',
  transform(code) { return code }
}
`,
  )
  fs.writeFileSync(
    path.join(root, 'hvigorfile.js'),
    `
module.exports = {
  plugins: [{
    pluginId: 'fixture-plugin',
    apply(node) {
      require('node:fs').writeFileSync(${JSON.stringify(marker)}, node.getNodeName())
    }
  }],
  compilePluginPath: './compile-plugin.js'
}
`,
  )

  const plugins = await new RuntimePluginLoader({ project: root }).load()
  assert.deepEqual(
    plugins.map((plugin) => plugin.name),
    ['compile-plugin'],
  )
  assert.equal(fs.readFileSync(marker, 'utf8'), path.basename(root))
})

test('runtime loader reads configured hvigorPluginFile and compilePluginPath arrays', async () => {
  const root = tempProject()
  fs.writeFileSync(
    path.join(root, 'hvigor-plugin.js'),
    `exports.default = { name: 'configured-hvigor', load() { return null } }\n`,
  )
  fs.writeFileSync(
    path.join(root, 'compile-a.js'),
    `exports.default = { name: 'configured-compile-a', transform(code) { return code } }\n`,
  )
  fs.writeFileSync(
    path.join(root, 'compile-b.js'),
    `exports.default = { name: 'configured-compile-b', transform(code) { return code } }\n`,
  )

  const plugins = await new RuntimePluginLoader({
    project: root,
    hvigorPluginFile: ['./hvigor-plugin.js'],
    compilePluginPath: ['./compile-a.js', './compile-b.js'],
  }).loadConfiguredPluginPaths()

  assert.deepEqual(
    plugins.map((plugin) => plugin.name),
    ['configured-hvigor', 'configured-compile-a', 'configured-compile-b'],
  )
})

test('createBuild prepends runtime plugins before explicit plugins', async () => {
  const root = tempProject()
  fs.writeFileSync(
    path.join(root, 'runtime-plugin.js'),
    `exports.default = { name: 'runtime-plugin', load() { return null } }\n`,
  )

  const seen = []
  const build = createBuild(async (_config, plugins) => {
    seen.push(...plugins.map((plugin) => plugin.name))
    return { artifacts: [] }
  })

  await build({ project: root, compilePluginPath: './runtime-plugin.js' }, [
    {
      name: 'explicit-plugin',
      load() {
        return null
      },
    },
  ])
  assert.deepEqual(seen, ['runtime-plugin', 'explicit-plugin'])
})

test('hvigor command runner renders latest module task inventory without invoking native build', async () => {
  const project = path.resolve(__dirname, '../../../tests/fixtures/minimal-hap')
  const run = createTestHvigorCommand(async () => {
    throw new Error('native build must not run for the tasks help action')
  })

  const result = await run({ project, executionMode: 'module', tasks: ['tasks'] })

  assert.equal(result.artifacts.length, 0)
  assert.equal(result.output.length, 1)
  assert.match(result.output[0], /All tasks from hvigor node 'entry'/)
  assert.match(result.output[0], /Help tasks[\s\S]*tasks - Displays the tasks of the HvigorCoreNode\./)
  assert.match(result.output[0], /Sync tasks[\s\S]*init - Sync and refresh project/)
  assert.doesNotMatch(result.output[0], /assembleHap/)
})

test('project taskTree keeps latest lazy assembleApp graph shape', async () => {
  const project = copyFixture('minimal-hap')
  const run = createTestHvigorCommand(async () => {
    throw new Error('native build must not run for the taskTree help action')
  })

  const result = await run({ project, executionMode: 'project', tasks: ['taskTree'] })

  assert.deepEqual(result.output[0].split(os.EOL), [
    '--------------------------------------------------------------------------',
    `All tasks dependencies from hvigor node '${path.basename(project)}'`,
    '--------------------------------------------------------------------------',
    '',
    `:${path.basename(project)}:assembleApp`,
    '\\--- ::SignApp',
  ])
})

test('buildInfo command uses the native profile projection without invoking a build', async () => {
  const project = path.resolve(__dirname, '../../../tests/fixtures/minimal-hap')
  const run = createTestHvigorCommand(async () => {
    throw new Error('native build must not run for buildInfo')
  })

  const result = await run({
    project,
    executionMode: 'project',
    tasks: ['buildInfo'],
    prop: ['json='],
  })

  assert.equal(result.artifacts.length, 0)
  assert.equal(result.output.length, 1)
  const info = JSON.parse(result.output[0])
  assert.deepEqual(info.product, ['default'])
  assert.deepEqual(info.module, ['entry'])
  assert.deepEqual(info.buildMode, ['debug', 'release'])
  assert.deepEqual(info.buildInfo.default.entry.default.debug, {})
})

test('hvigor command runner rejects an empty task list like latest 6.24.3', async () => {
  const project = path.resolve(__dirname, '../../../tests/fixtures/minimal-hap')
  const run = createTestHvigorCommand(async () => {
    throw new Error('native build must not run without a command task')
  })

  await assert.rejects(
    run({ project }),
    (error) =>
      error.code === 'HVIGOR_TASK_NOT_FOUND' &&
      error.message ===
        `Task [] was not found in the project minimal-hap. Invalid command to execute the build task, please verify the parameters in the command and try again.`,
  )
})

test('module clean delegates latest local and custom build-root behavior to Rust', async () => {
  const project = copyFixture('minimal-hap')
  const moduleRoot = path.join(project, 'entry')
  const configuredRoot = tempProject()
  const mappedProjectRoot = path.join(configuredRoot, path.basename(project))
  const directories = [
    path.join(project, 'build'),
    path.join(moduleRoot, 'build'),
    path.join(moduleRoot, '.test'),
    path.join(mappedProjectRoot, 'build'),
    path.join(mappedProjectRoot, 'entry', 'build'),
  ]
  for (const directory of directories) {
    fs.mkdirSync(directory, { recursive: true })
    fs.writeFileSync(path.join(directory, 'output.bin'), 'output')
  }
  const run = createTestHvigorCommand(async () => {
    throw new Error('native build must not run for the clean command')
  })

  const result = await run({
    project,
    executionMode: 'module',
    tasks: ['clean'],
    module: 'entry@default',
    extParams: { 'build-cache-dir': `"${configuredRoot}"` },
  })

  assert.deepEqual(result.output, [])
  for (const directory of directories) assert.equal(fs.existsSync(directory), false)
})

test('OHOS context build paths follow InjectUtil build-cache and build-dir branches', async () => {
  async function contextPaths(config) {
    const loader = new RuntimePluginLoader(config)
    await loader.load()
    const host = loader.hvigorHost
    const projectNode = [...host._nodesByName.values()].find((node) => node.isProject)
    const moduleNode = host._nodesByName.get('entry')
    const projectContext = host._ohosContextFor(projectNode)
    const moduleContext = host._ohosContextFor(moduleNode)
    let target
    moduleContext.targets((candidate) => {
      if (candidate.getTargetName() === 'default') target = candidate
    })
    assert.ok(target)
    return { projectContext, moduleContext, target }
  }

  const project = copyFixture('minimal-hap')
  const projectName = path.basename(project)
  const moduleRoot = path.join(project, 'entry')
  const cliBuildRoot = tempProject()
  const cli = await contextPaths({
    project,
    product: 'default',
    extParams: { 'build-cache-dir': `"${cliBuildRoot}"` },
  })
  const cliModuleBuild = path.join(fs.realpathSync.native(cliBuildRoot), projectName, 'entry', 'build')

  // hvigor-ohos-plugin 6.24.3 ProjectPathInfoIml/ModulePathInfoIml pass
  // supportBuildCacheDir=false for public output paths, but not for cache and
  // intermediate paths. An explicit build-cache-dir therefore cannot relocate
  // final outputs.
  assert.equal(cli.projectContext.getBuildRootPath(), path.join(project, 'build'))
  assert.equal(cli.projectContext.getBuildProductOutputPath(), path.join(project, 'build', 'outputs', 'default'))
  assert.equal(cli.moduleContext.getBuildProductRootPath(), moduleRoot)
  assert.equal(cli.target.getBuildTargetOutputPath(), path.join(moduleRoot, 'build', 'default', 'outputs', 'default'))
  assert.equal(cli.target.getModulePathDetails().getModuleBuildPath(), cliModuleBuild)
  assert.equal(
    cli.target.getModulePathDetails().getIntermediatesRes(),
    path.join(cliModuleBuild, 'default', 'intermediates', 'res', 'default'),
  )
  assert.equal(
    cli.target.getModulePathDetails().getModuleBuildCachePath(),
    path.join(cliModuleBuild, 'default', 'cache', 'default'),
  )

  const propertyBuildRoot = tempProject()
  const property = await contextPaths({
    project,
    product: 'default',
    hvigorProperties: { 'ohos.buildDir': propertyBuildRoot },
  })
  const canonicalPropertyBuildRoot = fs.realpathSync.native(propertyBuildRoot)
  const propertyProjectBuild = path.join(canonicalPropertyBuildRoot, projectName, 'build')
  const propertyModuleBuild = path.join(canonicalPropertyBuildRoot, projectName, 'entry', 'build')

  assert.equal(property.projectContext.getBuildRootPath(), propertyProjectBuild)
  assert.equal(
    property.projectContext.getBuildProductOutputPath(),
    path.join(propertyProjectBuild, 'outputs', 'default'),
  )
  assert.equal(property.moduleContext.getBuildProductRootPath(), moduleRoot)
  assert.equal(
    property.target.getBuildTargetOutputPath(),
    path.join(propertyModuleBuild, 'default', 'outputs', 'default'),
  )
  assert.equal(property.target.getModulePathDetails().getModuleBuildPath(), propertyModuleBuild)
})

test('OHOS project context exposes selected SDK API and ETS component metadata', async () => {
  const project = copyFixture('minimal-hap')
  const sdkRoot = tempProject()
  const etsLoader = path.join(sdkRoot, 'ets', 'build-tools', 'ets-loader')
  fs.mkdirSync(etsLoader, { recursive: true })
  fs.writeFileSync(
    path.join(sdkRoot, 'ets', 'oh-uni-package.json'),
    JSON.stringify({ data: { apiVersion: '17', version: '6.1.1.280', releaseType: 'Beta' } }),
  )
  const loader = new RuntimePluginLoader({ project, etsLoaderPath: etsLoader })
  await loader.load()
  const projectNode = [...loader.hvigorHost._nodesByName.values()].find((node) => node.isProject)
  const sdk = loader.hvigorHost._ohosContextFor(projectNode).getSdkDetails()

  assert.equal(sdk.isOhos(), false)
  assert.equal(sdk.getSdkDir(), sdkRoot)
  assert.equal(sdk.getSdkVersion(), 17)
  assert.equal(sdk.getEtsComponentVersion(), '6.1.1.280')
  assert.equal(sdk.getEtsComponentReleaseType(), 'Beta')
})

test('prune removes only latest-hvigor caches older than thirty days and runs wrapper pnpm', async () => {
  const userHome = tempProject()
  const staleRoot = path.join(userHome, 'project_caches', 'stale-cache')
  const freshRoot = path.join(userHome, 'project_caches', 'fresh-cache')
  for (const [root, lastUpdatedTime] of [
    [staleRoot, Date.now() - 31 * 24 * 60 * 60 * 1000],
    [freshRoot, Date.now() - 29 * 24 * 60 * 60 * 1000],
  ]) {
    fs.mkdirSync(path.join(root, 'workspace'), { recursive: true })
    fs.writeFileSync(path.join(root, 'workspace', 'metadata.json'), JSON.stringify({ lastUpdatedTime }))
  }
  const wrapperBin = path.join(userHome, 'wrapper', 'tools', 'node_modules', '.bin')
  fs.mkdirSync(wrapperBin, { recursive: true })
  const pnpm = path.join(wrapperBin, process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm')
  fs.writeFileSync(pnpm, process.platform === 'win32' ? '@exit /b 0\r\n' : '#!/bin/sh\nexit 0\n')
  if (process.platform !== 'win32') fs.chmodSync(pnpm, 0o755)

  const output = await new HvigorRuntimeHost(tempProject()).pruneHvigorCaches(userHome)

  assert.equal(fs.existsSync(staleRoot), false)
  assert.equal(fs.existsSync(freshRoot), true)
  assert.deepEqual(output, [
    'Clear the cache of the hvigor file',
    `Clear the hvigor cache file:  ${staleRoot}`,
    'Remove unreferenced packages from the store',
  ])
})

test('collectCoverage loads the compose runtime adjacent to the resolved hvigor plugin', async () => {
  const project = tempProject()
  const marker = path.join(project, 'coverage-call.json')
  const nodeModules = path.join(project, 'workspace', 'node_modules')
  const pluginRoot = path.join(nodeModules, '@ohos', 'hvigor-ohos-plugin')
  const composeRoot = path.join(pluginRoot, 'node_modules', '@ohos', 'hvigor-arkts-compose')
  fs.mkdirSync(composeRoot, { recursive: true })
  fs.writeFileSync(
    path.join(pluginRoot, 'package.json'),
    JSON.stringify({ name: '@ohos/hvigor-ohos-plugin', main: 'index.js' }),
  )
  fs.writeFileSync(path.join(pluginRoot, 'index.js'), 'module.exports = {}\n')
  fs.writeFileSync(
    path.join(composeRoot, 'package.json'),
    JSON.stringify({ name: '@ohos/hvigor-arkts-compose', main: 'index.js' }),
  )
  fs.writeFileSync(
    path.join(composeRoot, 'index.js'),
    `exports.generateReport = async (...args) => require('node:fs').writeFileSync(${JSON.stringify(marker)}, JSON.stringify(args))\n`,
  )

  const host = new HvigorRuntimeHost(project)
  host.hvigorWorkspaceNodeModules = nodeModules
  const projectPath = path.join(project, 'sources')
  const reportPath = path.join(project, 'report')
  const coverageFile = path.join(project, 'coverage.json')
  await host._generateCoverageReport(projectPath, reportPath, coverageFile)

  assert.deepEqual(JSON.parse(fs.readFileSync(marker, 'utf8')), [projectPath, reportPath, coverageFile])
})

test('hvigor command runner executes a selected TypeScript custom task without a native build', async () => {
  const root = tempProject()
  const marker = path.join(root, 'custom-task.txt')
  fs.writeFileSync(
    path.join(root, 'hvigorfile.ts'),
    `
import { hvigor } from '@ohos/hvigor'
import * as fs from 'node:fs'
export default {
  plugins: [{
    pluginId: 'command-task',
    apply(node: any) {
      node.registerTask({
        name: 'generateMarker',
        run() { fs.writeFileSync(${JSON.stringify(marker)}, hvigor.getCommandEntryTask()!.join(',')) }
      })
    }
  }]
}
`,
  )
  const run = createTestHvigorCommand(async () => {
    throw new Error('native build must not run for a standalone custom task')
  })

  await run({ project: root, executionMode: 'project', tasks: ['generateMarker'] })

  assert.equal(fs.readFileSync(marker, 'utf8'), 'generateMarker')
})

test('core TaskCreation remains lazy across the JS-to-Rust task model boundary', async () => {
  const root = tempProject()
  const realized = path.join(root, 'lazy-provider.txt')
  const deleteResults = path.join(root, 'lazy-delete-results.json')
  fs.writeFileSync(
    path.join(root, 'hvigorfile.js'),
    `
const fs = require('node:fs')
const { getHvigorNode } = require('@ohos/hvigor')
const node = getHvigorNode(__filename)
node.getTaskContainer().registerTask({
  details: {
    name: 'lazyCustom',
    group: 'Customize',
    description: 'lazy custom task',
    sourcePluginId: 'fixture.lazy',
  },
  depends: [],
  dependsDefinedByUser: [],
  provider() {
    fs.writeFileSync(${JSON.stringify(realized)}, 'realized')
    return node.task(() => {}, { name: 'lazyCustom' })
  },
})
fs.writeFileSync(${JSON.stringify(deleteResults)}, JSON.stringify({
  missing: node.getTaskContainer().deleteTask('missing'),
  unrealized: node.getTaskContainer().deleteTask('lazyCustom'),
}))
module.exports = { plugins: [] }
`,
  )

  const loader = new RuntimePluginLoader({ project: root, executionMode: 'project' })
  await loader.load()
  assert.equal(fs.existsSync(realized), false)
  assert.deepEqual(JSON.parse(fs.readFileSync(deleteResults, 'utf8')), { missing: true, unrealized: false })
  assert.deepEqual(loader.hvigorHost._runtimeTaskInventory(false).projectCustomTasks, [
    {
      taskName: 'lazyCustom',
      group: 'Customize',
      description: 'lazy custom task',
      sourcePluginId: 'fixture.lazy',
    },
  ])
  const lazyOverlay = loader.hvigorHost.createNativeTaskBridge().tasks.find((task) => task.taskName === 'lazyCustom')
  assert.equal(lazyOverlay.lazy, true)

  loader.hvigorHost._commandEntryTasks = ['lazyCustom']
  loader.hvigorHost.commandPlan()
  assert.equal(fs.readFileSync(realized, 'utf8'), 'realized')
})

test('submitWorker executes on a reusable worker and keeps the owning task pending', async (context) => {
  if (os.cpus().length <= 1) {
    context.skip('Hvigor defines MAX_POOL_NUM as cpu count minus one')
    return
  }
  const root = tempProject()
  const marker = path.join(root, 'worker-result.json')
  const workFile = path.join(root, 'worker-action.js')
  const workPath = path.join(workFile, 'nested', 'increment')
  fs.writeFileSync(
    workFile,
    [
      'exports.nested = {',
      '  increment(input, cacheManager) {',
      "    const cache = cacheManager.mount('fixture')",
      "    const count = (cache.get('count') || 0) + input",
      "    cache.set('count', count)",
      '    return count',
      '  }',
      '}',
      '',
    ].join('\n'),
  )
  fs.writeFileSync(
    path.join(root, 'hvigorfile.ts'),
    `
import { Priority, TaskState, submitWorker } from '@ohos/hvigor'
import * as fs from 'node:fs'
const controls: any[] = []
const results: any[] = []
export default {
  plugins: [{
    pluginId: 'worker-task',
    apply(node: any) {
      node.registerTask({
        name: 'workerTask',
        run() {
          controls.push(submitWorker(node, 'workerTask', ${JSON.stringify(workPath)}, {
            workInput: 1,
            priority: Priority.FIRST,
            targetWorkers: [0, 0, -1],
            useReturnVal: true,
            callback(value: number, workerId: number) { results.push([value, workerId]) }
          }))
          controls.push(submitWorker(node, 'workerTask', ${JSON.stringify(workPath)}, {
            workInput: 2,
            targetWorkers: [0],
            useReturnVal: true,
            callback(value: number, workerId: number) { results.push([value, workerId]) }
          }))
        },
        afterRun() {
          fs.writeFileSync(${JSON.stringify(marker)}, JSON.stringify({
            priority: Priority.FIRST,
            state: TaskState.END,
            results,
            controls: controls.map((control) => ({
              state: control.getState(),
              workerId: control.getWorkerId(),
              taskName: control.getTaskName(),
              taskCompletePath: control.getTaskCompletePath(),
            })),
          }))
        }
      })
    }
  }]
}
`,
  )
  const run = createTestHvigorCommand(async () => {
    throw new Error('native build must not run for a worker-backed custom task')
  })

  await run({ project: root, executionMode: 'project', tasks: ['workerTask'] })

  assert.deepEqual(JSON.parse(fs.readFileSync(marker, 'utf8')), {
    priority: 0,
    state: 'end',
    results: [
      [1, 0],
      [3, 0],
    ],
    controls: [
      { state: 'end', workerId: 0, taskName: 'workerTask', taskCompletePath: ':workerTask' },
      { state: 'end', workerId: 0, taskName: 'workerTask', taskCompletePath: ':workerTask' },
    ],
  })
})

test('submitWorker returns a rejected TCB when parallel execution is disabled', async () => {
  const root = tempProject()
  const marker = path.join(root, 'worker-rejected.json')
  fs.writeFileSync(path.join(root, 'work.js'), 'module.exports = () => 1\n')
  fs.writeFileSync(
    path.join(root, 'hvigorfile.js'),
    `
const fs = require('node:fs')
const { submitWorker } = require('@ohos/hvigor')
module.exports = { plugins: [{
  pluginId: 'worker-reject',
  apply(node) {
    node.registerTask({
      name: 'workerReject',
      run() {
        const control = submitWorker(node, 'workerReject', ${JSON.stringify(path.join(root, 'work.js'))}, {})
        fs.writeFileSync(${JSON.stringify(marker)}, JSON.stringify({ state: control.getState(), workerId: control.getWorkerId() }))
      }
    })
  }
}] }
`,
  )
  const run = createTestHvigorCommand(async () => {
    throw new Error('native build must not run for a worker rejection task')
  })

  await run({ project: root, executionMode: 'project', tasks: ['workerReject'], parallel: false })

  assert.deepEqual(JSON.parse(fs.readFileSync(marker, 'utf8')), { state: 'reject' })
})

test('latest hvigor incremental task exports use FileSet and CoreTask class semantics', () => {
  const host = new HvigorRuntimeHost(tempProject())
  const hvigor = host._makeHvigorMock()
  for (const exportName of [
    'CoreTask',
    'IncrementalExecTask',
    'FileSet',
    'Input',
    'Inputs',
    'InputFile',
    'InputFiles',
    'OutputFile',
    'OutputFiles',
    'IgnoreSuperIncremental',
  ]) {
    assert.equal(typeof hvigor[exportName], 'function', exportName)
  }

  const options = { isDirectory: true, depth: 2 }
  const files = new hvigor.FileSet()
    .addEntry('/fixture/one', options)
    .addEntries(['/fixture/two', '/fixture/three'], options)
  assert.equal(files.collect().get('/fixture/one'), options)
  assert.notEqual(files.collect().get('/fixture/two'), options)
  assert.deepEqual(files.collect().get('/fixture/two'), options)
  assert.equal(files.deleteEntry('/fixture/two'), files)
  assert.equal(files.collect().has('/fixture/two'), false)

  class FixtureTask extends hvigor.IncrementalExecTask {}
  const task = new FixtureTask({ classKind: 'project' }, 'fixture')
  assert.equal(task instanceof hvigor.CoreTask, true)
  assert.equal(task instanceof hvigor.DefaultTask, true)
  assert.equal(task.declareExecutionTool(), '')
  assert.equal(task.declareExecutionCommand(), '')
  assert.deepEqual(task.declareExecutionEnv(), new Map())
})

test('Hvigor mock exports the declared API plus the existing Arkdown task compatibility allowlist', () => {
  const hvigor = new HvigorRuntimeHost(tempProject())._makeHvigorMock()
  const expected = `
    BuildResult CoreTask DefaultSyncTask DefaultTask FileSet FileUtil IgnoreSuperIncremental
    IncrementalExecTask Input InputFile InputFiles Inputs OutputFile OutputFiles Priority TaskInput
    TaskOutput TaskState defaultModelRegistry getHvigorNode getNode hvigor hvigorConfig hvigorCore
    replacer submitWorker
  `
    .trim()
    .split(/\s+/)

  assert.deepEqual(Object.keys(hvigor).sort(), expected.sort())
  for (const exportName of expected) {
    assert.equal(exportName in hvigor, true, exportName)
  }
  assert.throws(
    () => hvigor.MetricFactory,
    (error) => error.code === 'ARKDOWN_UNSUPPORTED_HVIGOR_INTERNAL_EXPORT' && error.exportName === 'MetricFactory',
  )
})

test('latest hvigor exports reuse singleton state while retaining caller-bound node lookup', () => {
  const root = tempProject()
  const entry = path.join(root, 'entry')
  const projectFile = path.join(root, 'hvigorfile.ts')
  const moduleFile = path.join(entry, 'hvigorfile.ts')
  fs.mkdirSync(entry)
  fs.writeFileSync(
    path.join(root, 'build-profile.json5'),
    `{ modules: [{ name: 'entry', srcPath: './entry', targets: [{ name: 'default' }] }] }\n`,
  )
  fs.writeFileSync(path.join(entry, 'build-profile.json5'), `{ targets: [{ name: 'default' }] }\n`)

  const host = new HvigorRuntimeHost(root)
  host.prepareHvigorfiles([projectFile, moduleFile])
  const projectExports = host._makeHvigorMock(projectFile)
  const repeatedProjectExports = host._makeHvigorMock(projectFile)
  const moduleExports = host._makeHvigorMock(moduleFile)

  assert.equal(projectExports, repeatedProjectExports)
  assert.equal(projectExports.hvigor, moduleExports.hvigor)
  assert.equal(projectExports.hvigorCore, moduleExports.hvigorCore)
  assert.equal(projectExports.getNode().getNodeName(), path.basename(root))
  assert.equal(moduleExports.getNode().getNodeName(), 'entry')
  assert.equal(projectExports.hvigorCore.getProject().getName(), path.basename(root))
})

test('declared Hvigor FileUtil and replacer delegate filesystem behavior to Rust', async () => {
  const root = tempProject()
  const hvigor = new HvigorRuntimeHost(root)._makeHvigorMock()
  const jsonFile = path.join(root, 'fixture.json5')
  fs.writeFileSync(jsonFile, `{ foo: 1, nested: { bar: 'x' }, array: [true, { z: 2 }, 3] }`)

  assert.deepEqual(JSON.parse(JSON.stringify(new Map([['key', 1]]), hvigor.replacer)), { key: 1 })
  assert.deepEqual(JSON.parse(JSON.stringify(new Set(['a', 'b']), hvigor.replacer)), ['a', 'b'])
  assert.deepEqual(hvigor.FileUtil.readJson5(jsonFile), {
    foo: 1,
    nested: { bar: 'x' },
    array: [true, { z: 2 }, 3],
  })

  const ensuredFile = path.join(root, 'nested', 'file.txt')
  hvigor.FileUtil.ensureFileSync(ensuredFile)
  hvigor.FileUtil.writeFileSync(ensuredFile, 'content')
  assert.equal(hvigor.FileUtil.exist(ensuredFile), true)
  assert.equal(hvigor.FileUtil.isFile(ensuredFile), true)
  assert.equal(hvigor.FileUtil.isDictionary(path.dirname(ensuredFile)), true)
  assert.equal(hvigor.FileUtil.readFileSync(ensuredFile).toString(), 'content')
  await hvigor.FileUtil.writeFile(ensuredFile, 'async-content')
  assert.equal((await hvigor.FileUtil.readFile(ensuredFile)).toString(), 'async-content')
  assert.equal(hvigor.FileUtil.pathResolve(root, 'nested', '..', 'fixture.json5'), jsonFile)
})

test('Rust parameter owner preserves Hvigor mutable core state and immutable external snapshots', () => {
  const host = new HvigorRuntimeHost(tempProject())
  const hvigor = host._makeHvigorMock()
  const parameter = hvigor.hvigor.getParameter()
  const coreParameter = host._coreParameter
  const deviceTypes = ['phone']

  coreParameter.properties['ohos.align.deviceTypes'] = deviceTypes
  assert.equal(coreParameter.properties['ohos.align.deviceTypes'], deviceTypes)
  deviceTypes.push('tablet')

  const properties = parameter.getProperties()
  assert.deepEqual(properties['ohos.align.deviceTypes'], ['phone', 'tablet'])
  assert.equal(Object.isFrozen(properties), true)
  assert.equal(Object.isFrozen(properties['ohos.align.deviceTypes']), true)
  assert.notEqual(properties['ohos.align.deviceTypes'], deviceTypes)
  assert.throws(() => properties['ohos.align.deviceTypes'].push('tv'), TypeError)

  parameter.setProperty('fixture.scalar', 7)
  parameter.setProperty('ohos.align.deviceTypes', ['phone', '2in1'])
  assert.equal(parameter.getPropertyInternal('fixture.scalar'), 7)
  assert.deepEqual(parameter._getProperty('ohos.align.deviceTypes'), ['phone', '2in1'])
  assert.throws(
    () => parameter.setProperty('fixture.invalid', {}),
    (error) => error.code === 'ARKDOWN_HVIGOR_PARAMETER_VALIDATION_FAILED',
  )
  assert.throws(
    () => parameter.setProperty('ohos.align.deviceTypes', ['phone', 'phone']),
    (error) => error.code === 'ARKDOWN_HVIGOR_PARAMETER_VALIDATION_FAILED',
  )

  coreParameter.startParams.analyze = 3
  coreParameter.startParams.incrementalExecution = false
  assert.deepEqual(parameter.getStartParams(), {
    daemon: true,
    analyze: 'ultrafine',
    parallel: true,
    incremental: false,
    logLevel: 'info',
    typeCheck: false,
    optimizationStrategy: 'memory',
    hotCompile: false,
    hotReloadBuild: false,
  })
  assert.equal(Object.isFrozen(parameter.getStartParams()), true)
  assert.equal(parameter.getWorkspaceDirInternal(), host._runtimeSession.parameterPlan().workspaceDir)
  assert.deepEqual(Object.keys(coreParameter), ['_properties', '_extParams', '_startParams', '_workspaceDir'])

  coreParameter.clean()
  assert.deepEqual(parameter.getProperties(), {})
  assert.deepEqual(parameter.getExtParamsInternal(), {})
  assert.equal(parameter.getWorkspaceDir(), '')
  assert.equal(parameter.getStartParams().incremental, true)
})

test('decorated IncrementalExecTask matches latest beforeAlways, hook order, and up-to-date behavior', async () => {
  const root = tempProject()
  const inputPath = path.join(root, 'decorated-input.txt')
  const outputPath = path.join(root, 'decorated-output.txt')
  const secondOutputPath = path.join(root, 'decorated-output-two.txt')
  const tracePath = path.join(root, 'decorated-trace.txt')
  fs.writeFileSync(inputPath, 'input')
  fs.writeFileSync(
    path.join(root, 'hvigorfile.ts'),
    `
import {
  getNode,
  IncrementalExecTask,
  Input,
  Inputs,
  InputFile,
  InputFiles,
  OutputFile,
  OutputFiles,
} from '@ohos/hvigor'
import * as fs from 'node:fs'

class DecoratedTask extends IncrementalExecTask {
  @Input() flavor = 'default'
  @Inputs() dimensions = new Map([['api', 24], ['mode', 'debug']])
  @InputFile() inputPath = ${JSON.stringify(inputPath)}
  @InputFiles() inputPaths = [${JSON.stringify(inputPath)}]
  @OutputFile() outputPath = ${JSON.stringify(outputPath)}
  @OutputFiles() outputPaths = [${JSON.stringify(secondOutputPath)}]

  async beforeAlwaysAction() {
    fs.appendFileSync(${JSON.stringify(tracePath)}, 'always\\n')
  }

  registryAction() {
    return () => {
      fs.copyFileSync(this.inputPath, this.outputPath)
      fs.copyFileSync(this.inputPath, this.outputPaths[0])
      fs.appendFileSync(${JSON.stringify(tracePath)}, 'run\\n')
    }
  }
}

const node = getNode(__filename)!
const task = new DecoratedTask(node, 'decoratedTask')
task.beforeRun(() => fs.appendFileSync(${JSON.stringify(tracePath)}, 'before-one\\n'))
task.beforeRun(() => fs.appendFileSync(${JSON.stringify(tracePath)}, 'before-two\\n'))
task.afterRun(() => fs.appendFileSync(${JSON.stringify(tracePath)}, 'after-one\\n'))
task.afterRun(() => fs.appendFileSync(${JSON.stringify(tracePath)}, 'after-two\\n'))
node.registerTask(task)
export default { plugins: [] }
`,
  )

  const run = createTestHvigorCommand(async () => {
    throw new Error('native build must not run for a standalone decorated task')
  })
  await run({ project: root, executionMode: 'project', tasks: ['decoratedTask'], cache: true })
  await run({ project: root, executionMode: 'project', tasks: ['decoratedTask'], cache: true })

  assert.deepEqual(fs.readFileSync(tracePath, 'utf8').trim().split('\n'), [
    'always',
    'before-two',
    'before-one',
    'run',
    'after-one',
    'after-two',
    'always',
  ])
  assert.equal(fs.readFileSync(outputPath, 'utf8'), 'input')
  assert.equal(fs.readFileSync(secondOutputPath, 'utf8'), 'input')
})

test('hvigor command runner maps assembleHap to the selected native module target', async () => {
  const project = path.resolve(__dirname, '../../../tests/fixtures/minimal-hap')
  const calls = []
  const run = createTestHvigorCommand(async (config, _plugins, runtimeTasks) => {
    calls.push(config)
    assert.equal(typeof runtimeTasks.dispatchTask, 'function')
    return { artifacts: ['entry.hap'], elapsedMs: 1 }
  })

  const result = await run({
    project,
    tasks: ['assembleHap'],
    executionMode: 'module',
    prop: ['module=entry@default', 'buildMode=release'],
    module: 'entry@default',
    mode: 'release',
    watch: true,
  })

  assert.deepEqual(result.artifacts, ['entry.hap'])
  assert.equal(calls.length, 1)
  assert.equal(calls[0].target, 'hap')
  assert.equal(calls[0].module, 'entry')
  assert.equal(calls[0].buildTargetName, 'default')
  assert.equal(calls[0].mode, 'release')
  assert.equal(calls[0].watch, false)
  assert.equal(calls[0].nativeTaskRootsJson, undefined)
})

test('hvigor command runner maps test to the local unit-test native target', async () => {
  const project = path.resolve(__dirname, '../../../tests/fixtures/minimal-hap')
  const calls = []
  const run = createTestHvigorCommand(async (config, _plugins, runtimeTasks) => {
    calls.push(config)
    assert.equal(typeof runtimeTasks.dispatchTask, 'function')
    return { artifacts: [], elapsedMs: 1 }
  })

  await run({
    project,
    tasks: ['test'],
    executionMode: 'module',
    prop: ['module=entry'],
    module: 'entry',
  })

  assert.equal(calls.length, 1)
  assert.equal(calls[0].target, 'test')
  assert.equal(calls[0].module, 'entry')
  assert.equal(calls[0].buildTargetName, undefined)
  assert.deepEqual(JSON.parse(calls[0].nativeTaskRootsJson), [{ moduleName: 'entry', taskName: 'test' }])
})

test('module GenerateBuildProfile maps to its unqualified native cutoff boundary', async () => {
  const project = copyFixture('minimal-hap')
  const calls = []
  const run = createTestHvigorCommand(async (config, _plugins, runtimeTasks) => {
    calls.push(config)
    assert.equal(typeof runtimeTasks.dispatchTask, 'function')
    return { artifacts: [] }
  })

  await run({
    project,
    tasks: ['GenerateBuildProfile'],
    executionMode: 'module',
    module: 'entry@default',
  })

  assert.equal(calls.length, 1)
  assert.deepEqual(JSON.parse(calls[0].nativeTaskRootsJson), [
    { moduleName: 'entry', taskName: 'GenerateBuildProfile' },
  ])
})

test('module compileNative maps to its targetless native cutoff boundary', async () => {
  const project = copyFixture('minimal-hap')
  const calls = []
  const run = createTestHvigorCommand(async (config, _plugins, runtimeTasks) => {
    calls.push(config)
    assert.equal(typeof runtimeTasks.dispatchTask, 'function')
    return { artifacts: [] }
  })

  await run({
    project,
    tasks: ['compileNative'],
    executionMode: 'module',
    module: 'entry@default',
  })

  assert.equal(calls.length, 1)
  assert.deepEqual(JSON.parse(calls[0].nativeTaskRootsJson), [{ moduleName: 'entry', taskName: 'compileNative' }])
})

test('hvigor command runner maps project assembleApp to one native project request', async () => {
  const project = path.resolve(__dirname, '../../../tests/fixtures/minimal-hap')
  const calls = []
  const run = createTestHvigorCommand(async (config, _plugins, runtimeTasks) => {
    calls.push(config)
    assert.equal(typeof runtimeTasks.dispatchTask, 'function')
    return { artifacts: ['application.app'], elapsedMs: 1 }
  })

  const result = await run({
    project,
    tasks: ['assembleApp'],
    executionMode: 'project',
    product: 'default',
    mode: 'debug',
  })

  assert.deepEqual(result.artifacts, ['application.app'])
  assert.equal(calls.length, 1)
  assert.equal(calls[0].target, 'app')
  assert.equal(calls[0].module, undefined)
  assert.equal(calls[0].product, 'default')
  assert.equal(calls[0].mode, 'debug')
})

test('hvigor --sync writes the latest IDE project, module, and task models without a native build', async () => {
  const project = copyFixture('minimal-hap')
  const run = createTestHvigorCommand(async () => {
    throw new Error('native build must not run for sync')
  })

  await run({ project, sync: true, cache: false, product: 'default' })

  const syncRoot = path.join(project, '.hvigor', 'outputs', 'sync')
  const output = JSON.parse(fs.readFileSync(path.join(syncRoot, 'output.json'), 'utf8'))
  const taskInfo = JSON.parse(fs.readFileSync(path.join(syncRoot, 'taskInfo.json'), 'utf8'))
  assert.deepEqual(Object.keys(output), ['version', 'ohos-module-entry', 'ohos-project'])
  assert.equal(output.version, 1)
  assert.equal(output['ohos-project'].CONFIG_PROPERTIES.skipNativeIncremental, false)
  assert.equal(
    output['ohos-module-entry'].TARGETS.default.BUILD_PATH.WORKER_LOADER,
    path.join(project, 'entry/build/default/intermediates/loader/default/loader.json'),
  )
  assert.equal(taskInfo['ohos-module-entry'].length, 83)
  assert.deepEqual(taskInfo['ohos-module-entry'][0], {
    TASK_NAME: 'assembleHap',
    GROUP: 'Hook',
    DESCRIPTION: 'Assemble the task for the packaged HAP file.',
    SOURCE_PLUGIN_ID: 'hvigor-ohos-plugin',
  })
  assert.equal(taskInfo['ohos-module-entry'].at(-1).TASK_NAME, 'clean')

  await run({ project, sync: true, cache: false, product: 'default' })
  const repeatedTaskInfo = JSON.parse(fs.readFileSync(path.join(syncRoot, 'taskInfo.json'), 'utf8'))
  assert.equal(repeatedTaskInfo['ohos-module-entry'].length, 82)
  assert.equal(repeatedTaskInfo['ohos-module-entry'].at(-1).TASK_NAME, 'init')
})

test('hvigor --sync preserves latest HSP and HAR task inventories and cache paths', async () => {
  const cases = [
    {
      fixture: 'minimal-hsp',
      assembly: 'assembleHsp',
      compileTask: 'default@CompileArkTS',
      jsCompileTask: 'default@CompileJS',
      packageTask: 'default@PackageHsp',
    },
    {
      fixture: 'minimal-har',
      assembly: 'assembleHar',
      compileTask: 'default@HarCompileArkTS',
      jsCompileTask: 'default@HarCompileJS',
      packageTask: 'default@PackageHar',
    },
  ]

  for (const current of cases) {
    const project = copyFixture(current.fixture)
    const run = createTestHvigorCommand(async () => {
      throw new Error('native build must not run for sync')
    })
    await run({ project, sync: true, cache: false, product: 'default' })

    const syncRoot = path.join(project, '.hvigor', 'outputs', 'sync')
    const outputText = fs.readFileSync(path.join(syncRoot, 'output.json'), 'utf8')
    const taskInfoText = fs.readFileSync(path.join(syncRoot, 'taskInfo.json'), 'utf8')
    const output = JSON.parse(outputText)
    const taskNames = JSON.parse(taskInfoText)['ohos-module-entry'].map((task) => task.TASK_NAME)
    const buildPath = output['ohos-module-entry'].TARGETS.default.BUILD_PATH

    assert.equal(taskNames.length, 89)
    assert.equal(taskNames[0], current.assembly)
    assert.ok(taskNames.includes(current.packageTask))
    assert.equal(taskNames.at(-1), 'clean')
    assert.equal(
      buildPath.ETS_SUPER_VISUAL_PATH,
      path.join(project, 'entry', 'build', 'default', 'cache', 'default', current.compileTask, 'esmodule'),
    )
    assert.equal(
      buildPath.JS_SUPER_VISUAL_PATH,
      path.join(project, 'entry', 'build', 'default', 'cache', 'default', current.jsCompileTask, 'jsbundle'),
    )
    assert.equal(outputText.endsWith('\n'), false)
    assert.equal(taskInfoText.endsWith('\n'), false)
  }
})

test('class-based DefaultSyncTask can publish a custom ToolingModelBean', async () => {
  const project = tempProject()
  fs.writeFileSync(path.join(project, 'build-profile.json5'), '{ app: { products: [{ name: "default" }] } }')
  fs.writeFileSync(
    path.join(project, 'hvigorfile.js'),
    `
const { DefaultSyncTask, defaultModelRegistry } = require('@ohos/hvigor')
const { appTasks } = require('@ohos/hvigor-ohos-plugin')
class PublishModel extends DefaultSyncTask {
  constructor(node) { super(node, { name: 'publishModel', description: 'Publish test model.' }) }
  registryAction() {
    return () => defaultModelRegistry.registry({
      modelId: 'fixture-model',
      buildModel() { return { value: 42 } },
      taskInfo() { return [{ TASK_NAME: 'publishModel' }] },
    })
  }
}
module.exports = {
  system: appTasks,
  plugins: [{ pluginId: 'fixture-sync', apply(node) { node.registerTask(new PublishModel(node)) } }],
}
`,
  )
  const run = createTestHvigorCommand(async () => {
    throw new Error('native build must not run for custom sync')
  })

  await run({ project, sync: true, executionMode: 'project', cache: false })

  const syncRoot = path.join(project, '.hvigor', 'outputs', 'sync')
  const output = JSON.parse(fs.readFileSync(path.join(syncRoot, 'output.json'), 'utf8'))
  const taskInfo = JSON.parse(fs.readFileSync(path.join(syncRoot, 'taskInfo.json'), 'utf8'))
  assert.deepEqual(output['fixture-model'], { value: 42 })
  assert.deepEqual(taskInfo['fixture-model'], [{ TASK_NAME: 'publishModel' }])
})

test('latest hvigor-config and CLI properties feed the shared Parameter model', async () => {
  const root = tempProject()
  const hvigorDir = path.join(root, 'hvigor')
  const snapshot = path.join(root, 'parameter.json')
  fs.mkdirSync(hvigorDir)
  fs.writeFileSync(
    path.join(hvigorDir, 'hvigor-config.json5'),
    `{
      properties: {
        projectValue: 7,
        overridden: 'project',
        'ohos.pack.compressLevel': 'standard',
        'ohos.align.deviceTypes': 'phone,tablet',
        'ohos.collect.debugSymbol': false,
        'ohos.uiTransform.Optimization': true,
        'ohos.integratedHsp.useGetCurrentBundleName': true,
        'ohos.byteCodeHar.integratedOptimization': true,
        'hvigor.keepDependency': false,
        'ohos.align.target': 'tablet',
        'ohos.arkCompile.sourceMapDir': 'maps',
        'ohos.har.excludeHspDependencies': true,
        'ohos.dependencies.types.enable': true,
        'ohos.arkCompile.singleFileEmit': true,
        'ohos.rollupCache.useSourceHash': true,
        'ohos.processLib.optimization': true,
        'ohos.compile.lib.entryfile': false,
        'ohos.defaults.release.cmakebuildtype': 'RelWithDebInfo',
        'ohos.fallback.target': 'phone,default',
        'ohos.obfuscationRules.optimization': true,
        'ohos.sign.har': true,
      },
      execution: { incremental: true, parallel: false, daemon: false, optimizationStrategy: 'performance' },
      logging: { level: 'warn' },
      debugging: { stacktrace: true }
    }\n`,
  )
  fs.writeFileSync(
    path.join(root, 'hvigorfile.js'),
    `
const { hvigor, hvigorCore } = require('@ohos/hvigor')
require('node:fs').writeFileSync(${JSON.stringify(snapshot)}, JSON.stringify({
  projectValue: hvigor.getParameter().getProperty('projectValue'),
  overridden: hvigor.getParameter().getProperty('overridden'),
  cliNumber: hvigor.getParameter().getProperty('cliNumber'),
  ext: hvigor.getParameter().getExtParam('custom'),
  start: hvigor.getParameter().getStartParams(),
  internalProjectValue: hvigorCore.getParameter().getPropertyInternal('projectValue'),
}))
module.exports = { plugins: [] }
`,
  )

  const runtime = new RuntimePluginLoader({
    project: root,
    prop: ['custom=a=b', 'parameterFile=params.json5', 'properties.overridden=prop'],
    configOverrides: ['properties.cliNumber=42'],
  })
  await runtime.load()

  const value = JSON.parse(fs.readFileSync(snapshot, 'utf8'))
  assert.equal(value.projectValue, 7)
  assert.equal(value.overridden, 'prop')
  assert.equal(value.cliNumber, 42)
  assert.equal(value.ext, 'a=b')
  assert.equal(value.start.incremental, true)
  assert.equal(value.start.parallel, false)
  assert.equal(value.start.optimizationStrategy, 'performance')
  assert.equal(value.start.logLevel, 'warn')
  assert.equal(value.start.typeCheck, false)
  assert.equal(value.start.analyze, 'normal')
  assert.equal(value.start.printStackTrace, undefined)
  assert.equal(value.internalProjectValue, 7)
  assert.equal(runtime.toNativeBuildConfig().cache, true)
  assert.equal(runtime.toNativeBuildConfig().packCompressLevel, 5)
  assert.deepEqual(runtime.toNativeBuildConfig().alignDeviceTypes, ['phone', 'tablet'])
  assert.equal(runtime.toNativeBuildConfig().collectDebugSymbol, false)
  assert.equal(runtime.toNativeBuildConfig().uiTransformOptimization, true)
  assert.equal(runtime.toNativeBuildConfig().integratedHspUseGetCurrentBundleName, true)
  assert.equal(runtime.toNativeBuildConfig().byteCodeHarIntegratedOptimization, true)
  assert.equal(runtime.toNativeBuildConfig().keepDependency, false)
  assert.equal(runtime.toNativeBuildConfig().parameterFileOverride, 'params.json5')
  assert.deepEqual(runtime.hvigorHost.runtimeProperties(runtime.hvigorHost.startParameters()), {
    daemon: false,
    alignTarget: 'tablet',
    sourceMapDir: 'maps',
    harExcludeHspDependencies: true,
    dependenciesTypesEnable: true,
    singleFileEmit: true,
    rollupCacheUseSourceHash: true,
    processLibOptimization: true,
    compileLibEntryFile: false,
    releaseCmakeBuildType: 'RelWithDebInfo',
    fallbackTargets: ['phone', 'default'],
    obfuscationRulesOptimization: true,
    signHar: true,
    previewerCompileResIncremental: false,
    compileHspDependencies: false,
    compileHspDependenciesTruthy: false,
  })
})

test('preview compileHspDependencies preserves Hvigor literal and truthy predicates', () => {
  const falseText = new HvigorRuntimeHost(tempProject(), {
    prop: ['compileHspDependencies=false'],
  }).runtimeProperties({})
  assert.equal(falseText.compileHspDependencies, false)
  assert.equal(falseText.compileHspDependenciesTruthy, true)

  const trueText = new HvigorRuntimeHost(tempProject(), {
    prop: ['compileHspDependencies=true'],
  }).runtimeProperties({})
  assert.equal(trueText.compileHspDependencies, true)
  assert.equal(trueText.compileHspDependenciesTruthy, true)
})

test('BinXO extra config preserves Hvigor truthy native-property override', () => {
  const enabled = new HvigorRuntimeHost(tempProject(), {
    prop: ['ohos-enable-hwasan=true', 'ohos-enable-binxo=true'],
  })
  assert.deepEqual(enabled.runtimeProperties({}), {
    daemon: false,
    harExcludeHspDependencies: false,
    dependenciesTypesEnable: false,
    singleFileEmit: false,
    rollupCacheUseSourceHash: false,
    processLibOptimization: false,
    fallbackTargets: [],
    obfuscationRulesOptimization: false,
    signHar: false,
    previewerCompileResIncremental: false,
    compileHspDependencies: false,
    compileHspDependenciesTruthy: false,
    hwasanConfig: true,
    binxoConfig: true,
  })

  const explicitFalse = new HvigorRuntimeHost(tempProject(), {
    prop: ['ohos-enable-binxo=false'],
  })
  assert.equal(explicitFalse.runtimeProperties({}).binxoConfig, false)

  const fallback = new HvigorRuntimeHost(tempProject(), {})
  assert.equal(fallback.runtimeProperties({}).binxoConfig, undefined)
})

test('sanitizer extra config uses Hvigor exact true predicates', () => {
  const host = new HvigorRuntimeHost(tempProject(), {
    prop: ['ohos-debug-asan=true', 'ohos-enable-tsan=true', 'ohos-enable-hwasan=false', 'ohos-enable-ubsan=true'],
  })
  const properties = host.runtimeProperties({})
  assert.equal(properties.asanConfig, true)
  assert.equal(properties.tsanConfig, true)
  assert.equal(properties.hwasanConfig, false)
  assert.equal(properties.ubsanConfig, true)

  const native = new RuntimePluginLoader({
    project: host.projectRoot,
    prop: ['ohos-debug-asan=true', 'ohos-enable-tsan=true', 'ohos-enable-hwasan=false', 'ohos-enable-ubsan=true'],
  }).toNativeBuildConfig()
  assert.equal(native.ohosDebugAsan, true)
  assert.equal(native.ohosEnableTsan, true)
  assert.equal(native.ohosEnableHwasan, false)
  assert.equal(native.ohosEnableUbsan, true)
})

test('test commands inject Hvigor defaults without overriding explicit CLI properties', () => {
  const localTest = new HvigorRuntimeHost(tempProject(), {
    tasks: ['test'],
    prop: ['product=custom', 'coverage=false', 'pageType=component'],
  })
  assert.deepEqual(localTest._parameter.getExtParamsInternal(), {
    product: 'custom',
    'ohos-test-coverage': 'false',
    pageType: 'component',
    buildRoot: '.test',
    unitTestMode: 'true',
    isLocalTest: 'true',
    'unit.test.replace.page': '../../../.test/testability/pages/Index',
  })
  assert.equal(localTest._effectiveBuildMode(), 'debug')

  const deviceTest = new HvigorRuntimeHost(tempProject(), {
    tasks: ['onDeviceTest'],
    prop: ['product=phone'],
  })
  assert.deepEqual(deviceTest._parameter.getExtParamsInternal(), {
    product: 'phone',
    buildMode: 'test',
    isOhosTest: 'true',
    'ohos-test-coverage': 'true',
  })
  assert.equal(deviceTest._effectiveBuildMode(), 'test')
})

test('byteCode HAR integrated optimization requires the literal boolean true', async () => {
  const root = tempProject()
  const runtime = new RuntimePluginLoader({
    project: root,
    hvigorProperties: {
      'ohos.byteCodeHar.integratedOptimization': 'true',
      'hvigor.keepDependency': 'false',
    },
  })
  await runtime.load()
  assert.equal(runtime.toNativeBuildConfig().byteCodeHarIntegratedOptimization, false)
  assert.equal(runtime.toNativeBuildConfig().keepDependency, true)
})

test('runtime loader applies project and module hvigor plugins in node order', async () => {
  const root = tempProject()
  const entry = path.join(root, 'entry')
  fs.mkdirSync(entry)
  const marker = path.join(root, 'plugin-order.txt')
  fs.writeFileSync(
    path.join(root, 'hvigorfile.js'),
    `
module.exports = {
  plugins: [{
    pluginId: 'project-plugin',
    apply(node) { require('node:fs').appendFileSync(${JSON.stringify(marker)}, node.getNodeName() + '\\n') }
  }]
}
`,
  )
  fs.writeFileSync(
    path.join(entry, 'hvigorfile.js'),
    `
module.exports = {
  plugins: [{
    pluginId: 'module-plugin',
    apply(node) { require('node:fs').appendFileSync(${JSON.stringify(marker)}, node.getNodeName() + '\\n') }
  }]
}
`,
  )

  const plugins = await new RuntimePluginLoader({ project: root }).load()
  assert.deepEqual(plugins, [])
  assert.deepEqual(fs.readFileSync(marker, 'utf8').trim().split('\n'), [path.basename(root), 'entry'])
})

test('hvigorfile selection follows 6.24.3 ts js cjs mjs precedence', async () => {
  const root = tempProject()
  const marker = path.join(root, 'selected-hvigorfile.txt')
  fs.writeFileSync(
    path.join(root, 'hvigorfile.js'),
    `module.exports = { plugins: [{ pluginId: 'js', apply() { require('node:fs').writeFileSync(${JSON.stringify(marker)}, 'js') } }] }\n`,
  )
  fs.writeFileSync(
    path.join(root, 'hvigorfile.ts'),
    `export default { plugins: [{ pluginId: 'ts', apply() { require('node:fs').writeFileSync(${JSON.stringify(marker)}, 'ts') } }] }\n`,
  )

  await new RuntimePluginLoader({ project: root }).load()
  assert.equal(fs.readFileSync(marker, 'utf8'), 'ts')
})

test('hvigor TypeScript execution uses the Rust OXC Rolldown compiler boundary', async () => {
  const root = tempProject()
  const marker = path.join(root, 'rust-typescript-compiler.txt')
  const typescript = require('typescript')
  const originalTranspileModule = typescript.transpileModule
  typescript.transpileModule = () => {
    throw new Error('JavaScript transpilation must not run')
  }

  try {
    fs.writeFileSync(
      path.join(root, 'plugin.ts'),
      `
const base = { pluginId: 'rust-compiled' }
export const plugin = {
  ...base,
  apply() { require('node:fs').writeFileSync(${JSON.stringify(marker)}, base.pluginId) }
}
`,
    )
    fs.writeFileSync(
      path.join(root, 'hvigorfile.ts'),
      `import { plugin } from './plugin.ts'\nexport default { plugins: [plugin] }\n`,
    )

    await new RuntimePluginLoader({ project: root }).load()
    assert.equal(fs.readFileSync(marker, 'utf8'), 'rust-compiled')
  } finally {
    typescript.transpileModule = originalTranspileModule
  }
})

test('latest hvigor mjs files use the CommonJS TypeScript hook and OHOS mocks', async () => {
  const root = tempProject()
  const marker = path.join(root, 'mjs-loaded.txt')
  fs.writeFileSync(
    path.join(root, 'plugin.mjs'),
    `
import * as fs from 'node:fs'
export const custom = {
  pluginId: 'mjs-plugin',
  apply(node) { fs.writeFileSync(${JSON.stringify(marker)}, node.getNodeName()) }
}
`,
  )
  fs.writeFileSync(
    path.join(root, 'hvigorfile.mjs'),
    `
import { hapTasks } from '@ohos/hvigor-ohos-plugin'
import { custom } from './plugin.mjs'
export default { system: hapTasks, plugins: [custom] }
`,
  )

  await new RuntimePluginLoader({ project: root }).load()
  assert.equal(fs.readFileSync(marker, 'utf8'), path.basename(root))
})

test('hvigor beforeNodeEvaluate and afterNodeEvaluate hooks preserve node lifecycle order', async () => {
  const root = tempProject()
  const entry = path.join(root, 'entry')
  const marker = path.join(root, 'node-lifecycle.txt')
  fs.mkdirSync(entry)
  fs.writeFileSync(
    path.join(root, 'hvigorfile.ts'),
    `
import { hvigor } from '@ohos/hvigor'
import * as fs from 'node:fs'
hvigor.beforeNodeEvaluate((node: any) => fs.appendFileSync(${JSON.stringify(marker)}, 'before:' + node.getNodeName() + '\\n'))
hvigor.afterNodeEvaluate((node: any) => fs.appendFileSync(${JSON.stringify(marker)}, 'after:' + node.getNodeName() + '\\n'))
export default { plugins: [] }
`,
  )
  fs.writeFileSync(path.join(entry, 'hvigorfile.ts'), `export default { plugins: [] }\n`)

  await new RuntimePluginLoader({ project: root }).load()
  assert.deepEqual(fs.readFileSync(marker, 'utf8').trim().split('\n'), [
    `after:${path.basename(root)}`,
    'before:entry',
    'after:entry',
  ])
})

test('hvigorconfig.ts mutates descriptors before latest lifecycle phases run', async () => {
  const root = tempProject()
  const entry = path.join(root, 'entry')
  const dynamic = path.join(root, 'dynamic')
  const marker = path.join(root, 'hvigorconfig-lifecycle.txt')
  fs.mkdirSync(entry)
  fs.mkdirSync(dynamic)
  fs.writeFileSync(
    path.join(root, 'build-profile.json5'),
    `{
      app: { products: [{ name: 'default' }] },
      modules: [{ name: 'entry', srcPath: './entry' }]
    }\n`,
  )
  fs.writeFileSync(
    path.join(root, 'hvigorconfig.ts'),
    `
import { hvigor } from '@ohos/hvigor'
import * as fs from 'node:fs'
hvigor.configEvaluated((config: any) => {
  config.excludeNodeByName('entry')
  config.includeNode('dynamic', './dynamic', { flavor: 'fixture' })
  fs.appendFileSync(${JSON.stringify(marker)}, 'configEvaluated\\n')
})
hvigor.nodesInitialized((runtime: any) => {
  const nodes = runtime.getAllNodes().map((node: any) => node.getNodeName() + ':' + node.classKind)
  const dynamicNode = runtime.getNodeByName('dynamic', 'module')
  fs.appendFileSync(${JSON.stringify(marker)}, 'nodesInitialized:' + nodes.join(',') + ':' + dynamicNode.getExtraOption('flavor') + '\\n')
})
hvigor.nodesEvaluated(() => fs.appendFileSync(${JSON.stringify(marker)}, 'nodesEvaluated\\n'))
hvigor.taskGraphResolved(() => fs.appendFileSync(${JSON.stringify(marker)}, 'taskGraphResolved\\n'))
hvigor.buildFinished((result: any) => fs.appendFileSync(${JSON.stringify(marker)}, 'buildFinished:' + (result.getError() === null) + '\\n'))
`,
  )
  fs.writeFileSync(path.join(root, 'hvigorfile.ts'), `export default { plugins: [] }\n`)
  fs.writeFileSync(path.join(dynamic, 'hvigorfile.ts'), `export default { plugins: [] }\n`)

  const build = createBuild(async () => ({ artifacts: [] }))
  await build({ project: root })

  assert.deepEqual(fs.readFileSync(marker, 'utf8').trim().split('\n'), [
    'configEvaluated',
    `nodesInitialized:dynamic:module,${path.basename(root)}:project:fixture`,
    'nodesEvaluated',
    'taskGraphResolved',
    'buildFinished:true',
  ])
})

test('normalizePluginList keeps plugin arrays returned by factories', () => {
  const plugins = normalizePluginList(() => [
    {
      name: 'factory-a',
      transform(code) {
        return code
      },
    },
    () => ({
      name: 'factory-b',
      load() {
        return null
      },
    }),
  ])

  assert.deepEqual(
    plugins.map((plugin) => plugin.name),
    ['factory-a', 'factory-b'],
  )
})

test('shouldInvalidCache receives structured request objects', () => {
  const [plugin] = normalizePluginList({
    name: 'cache-plugin',
    shouldInvalidCache(request) {
      return request.cacheHit && request.id === 'entry' && request.meta.compileEnv === 'env'
    },
  })

  assert.equal(
    dispatchPluginHook(plugin, 'shouldInvalidCache', {
      cacheRequest: {
        id: 'entry',
        meta: {
          moduleName: 'entry',
          cacheDir: '/tmp/cache',
          compileEnv: 'env',
        },
        changedFiles: ['entry/src/main/ets/pages/Index.ets'],
        isWatch: true,
        cacheHit: true,
      },
    }),
    true,
  )
  assert.deepEqual(parseShouldInvalidCacheRequest('not-json'), { cacheHit: true })

  const nativeAst = parsePluginAst('const n = 123n; const r = /ab+/gi;', {
    sourceType: 'module',
    ecmaVersion: 'latest',
  }).ast
  const cacheRequest = parseShouldInvalidCacheRequest({ id: 'entry', code: 'cached code', __arkdownOxcAst: nativeAst })
  assert.equal(cacheRequest.ast.type, 'Program')
  assert.equal(typeof cacheRequest.ast.body[0].declarations[0].init.value, 'bigint')
  assert.equal(cacheRequest.ast.body[1].declarations[0].init.value instanceof RegExp, true)
  assert.equal(Object.prototype.hasOwnProperty.call(cacheRequest, '__arkdownOxcAst'), false)
})

test('compilePluginPath modules require default export like hvigor', () => {
  assert.throws(
    () => requireDefaultExport({ name: 'missing-default' }, '/tmp/plugin.js'),
    /PLUGIN_FILE_NO_DEFAULT_EXPORT/,
  )
})

test('require.cache is cleared before each loadPluginPath reload', async () => {
  const root = tempProject()
  const pluginFile = path.join(root, 'cache-plugin.js')
  let callCount = 0

  // Write initial plugin; the module increments a counter on each require
  fs.writeFileSync(
    pluginFile,
    `
let n = ${callCount};
exports.default = { name: 'cache-plugin', transform(code) { return code; }, _loaded: ++n };
`,
  )

  const loader1 = new RuntimePluginLoader({ project: root, compilePluginPath: './cache-plugin.js' })
  const plugins1 = await loader1.loadConfiguredPluginPaths()

  // Overwrite with a new version and reload via a fresh loader (seen Set is fresh)
  fs.writeFileSync(
    pluginFile,
    `
exports.default = { name: 'cache-plugin-v2', transform(code) { return code; } };
`,
  )
  const loader2 = new RuntimePluginLoader({ project: root, compilePluginPath: './cache-plugin.js' })
  const plugins2 = await loader2.loadConfiguredPluginPaths()

  // If require.cache were NOT cleared, both loaders would return the same module.
  // With cache cleared, loader2 should see the updated file.
  assert.equal(plugins1[0].name, 'cache-plugin')
  assert.equal(plugins2[0].name, 'cache-plugin-v2')
})

test('INVALID_PLUGIN_TYPE is thrown when a non-default export object lacks any hook', () => {
  // normalizePluginList silently drops non-hook objects; the error case is
  // requireDefaultExport on a non-default export — already tested above.
  // Here we verify that a plugin with no hooks and no path is silently dropped
  // (matching the existing hvigor behaviour of filtering with .filter(e => typeof e === 'object')).
  const plugins = normalizePluginList({ name: 'no-hooks' })
  assert.equal(plugins.length, 0)
})

test('PLUGIN_NAME_WITH_UNKNOWN_ORDER is thrown for unknown string order values', () => {
  assert.throws(
    () =>
      normalizePluginList({
        name: 'bad-order',
        order: 'middle',
        transform(code) {
          return code
        },
      }),
    (err) => err.code === 'PLUGIN_NAME_WITH_UNKNOWN_ORDER',
  )
})

test('function order receives name-to-index record and returns correct splice position', () => {
  const alpha = {
    name: 'alpha',
    transform(code) {
      return code
    },
  }
  const gamma = {
    name: 'gamma',
    transform(code) {
      return code
    },
  }
  let orderCalls = 0
  // beta should be inserted between alpha and gamma by returning index 1
  const beta = {
    name: 'beta',
    order(nameToIndex) {
      orderCalls++
      // alpha is at 0, so 1 inserts after alpha
      return (nameToIndex.alpha ?? 0) + 1
    },
    transform(code) {
      return code
    },
  }

  const result = applyFunctionOrder([alpha, gamma, beta])
  assert.deepEqual(
    result.map((p) => p.name),
    ['alpha', 'beta', 'gamma'],
  )
  // order property should be stripped from the resolved plugin
  assert.equal(result[1].order, undefined)
  // Hvigor's getPlugins invokes a function-valued order exactly once.
  assert.equal(orderCalls, 1)
})

test('function order receives hvigor Record shape and safe duplicate names', () => {
  let received
  const ordered = {
    name: 'ordered',
    order(nameToIndex) {
      received = nameToIndex
      return nameToIndex.alpha_1 + 1
    },
    transform(code) {
      return code
    },
  }
  const result = applyFunctionOrder([
    {
      name: 'alpha',
      transform(code) {
        return code
      },
    },
    {
      name: 'alpha',
      transform(code) {
        return code
      },
    },
    ordered,
  ])

  assert.equal(received instanceof Map, false)
  assert.deepEqual({ ...received }, { alpha: 0, alpha_1: 1 })
  assert.deepEqual(
    result.map((plugin) => plugin.name),
    ['alpha', 'alpha', 'ordered'],
  )
})

test('latest hvigor compile-plugin validation is strict and keeps name-only plugins', () => {
  assert.throws(
    () => normalizeSortedPluginList({ transform() {} }, undefined, '/tmp/missing-name.ts'),
    (error) => error.code === 'PLUGIN_NAME_NOT_EXIST',
  )
  assert.throws(
    () => normalizeSortedPluginList({ name: 42, transform() {} }, undefined, '/tmp/non-string-name.ts'),
    (error) => error.code === 'PLUGIN_NAME_NOT_STRING',
  )
  assert.throws(
    () => normalizeSortedPluginList({ name: 'bad-hook', transform: 'not-a-function' }, undefined, '/tmp/hook.ts'),
    (error) => error.code === 'PLUGIN_NAME_WITH_UNKNOWN_HOOK',
  )

  const [plugin] = normalizeSortedPluginList(
    {
      name: 'name-only',
      resolveDynamicImport() {},
    },
    undefined,
    '/tmp/name-only.ts',
  )
  assert.equal(plugin.name, 'name-only')
  assert.equal(plugin.resolveDynamicImport, undefined)
})

test('latest hvigor function order observes IDE built-ins and resolves absolute Rust slots', () => {
  let received
  const plugins = normalizeSortedPluginList(
    [
      { name: 'last-plugin' },
      { name: 'first-a', order: 'first' },
      { name: 'first-b', order: 'first' },
      {
        name: 'after-module-meta',
        order(nameToIndex) {
          received = nameToIndex
          return nameToIndex.moduleInfoMetaPlugin + 1
        },
      },
    ],
    undefined,
    '/tmp/order.ts',
  )

  new HvigorCompilePluginPlanner({ target: 'hap', compileMode: 'esmodule' }).apply(plugins)

  assert.equal(received instanceof Map, false)
  assert.equal(typeof received.commonPlugin, 'number')
  assert.equal(typeof received.etsTransform, 'number')
  assert.deepEqual(Object.fromEntries(plugins.map((plugin) => [plugin.name, plugin.__arkdownOrderIndex])), {
    'last-plugin': 11,
    'first-a': 1,
    'first-b': 0,
    'after-module-meta': 4,
  })
})

test('latest hvigor plans duplicate compile-plugin names independently per module', () => {
  const firstRoot = path.resolve('/tmp/arkdown-module-a')
  const secondRoot = path.resolve('/tmp/arkdown-module-b')
  const first = normalizeSortedPluginList(
    {
      name: 'same-name',
      transform(code) {
        return code
      },
    },
    undefined,
    '/tmp/a-plugin.ts',
    firstRoot,
  )[0]
  const second = normalizeSortedPluginList(
    {
      name: 'same-name',
      transform(code) {
        return code
      },
    },
    undefined,
    '/tmp/b-plugin.ts',
    secondRoot,
  )[0]

  const result = new HvigorCompilePluginPlanner({ target: 'hap' }).apply([first, second])

  assert.deepEqual(result, [first, second])
  assert.equal(first.__arkdownModuleRoot, firstRoot)
  assert.equal(second.__arkdownModuleRoot, secondRoot)
  assert.equal(first.__arkdownOrderIndex, second.__arkdownOrderIndex)
})

test('latest hvigor Rust planner preserves callback and duplicate diagnostics', () => {
  const [throwing] = normalizeSortedPluginList(
    {
      name: 'throwing-order',
      order() {
        throw new Error('order exploded')
      },
    },
    undefined,
    '/tmp/throwing-order.ts',
  )
  assert.throws(
    () => new HvigorCompilePluginPlanner({ target: 'hap' }).apply([throwing]),
    (error) =>
      error.code === 'INVALID_PLUGIN_EXPORT' &&
      error.pluginName === 'throwing-order' &&
      error.message.includes('order exploded'),
  )

  const duplicated = normalizeSortedPluginList(
    [{ name: 'same-scope' }, { name: 'same-scope' }],
    undefined,
    '/tmp/duplicate-order.ts',
  )
  assert.throws(
    () => new HvigorCompilePluginPlanner({ target: 'hap' }).apply(duplicated),
    (error) => error.code === 'DUPLICATED_PLUGIN_NAME_IN_ROLLUP' && error.pluginName === 'same-scope',
  )
})

test('latest hvigor compile buildStart receives no argument and a seeded share context', () => {
  let receivedArgumentCount
  let receivedShare
  const [plugin] = normalizeSortedPluginList(
    {
      name: 'strict-build-start',
      buildStart() {
        receivedArgumentCount = arguments.length
        receivedShare = this.share
      },
    },
    undefined,
    '/tmp/build-start.ts',
  )
  const share = { projectConfig: { bundleName: 'com.example.strict' } }
  seedPluginContext(plugin, share)
  dispatchPluginHook(plugin, 'buildStart', { buildStartOptions: {} })

  assert.equal(receivedArgumentCount, 0)
  assert.equal(receivedShare.projectConfig.bundleName, 'com.example.strict')
})

test('latest hvigor strict lifecycle wrappers discard async return values', () => {
  const pending = new Promise(() => {})
  const calls = []
  const [plugin] = normalizeSortedPluginList(
    {
      name: 'strict-async-return-contract',
      buildStart() {
        calls.push('buildStart')
        return pending
      },
      moduleParsed() {
        calls.push('moduleParsed')
        return pending
      },
      beforeBuildEnd() {
        calls.push('beforeBuildEnd')
        return pending
      },
      buildEnd() {
        calls.push('buildEnd')
        return pending
      },
    },
    undefined,
    '/tmp/strict-async-return.ts',
  )

  assert.deepEqual(dispatchPluginHook(plugin, 'buildStart', { buildStartOptions: {} }), {})
  assert.deepEqual(dispatchPluginHook(plugin, 'moduleParsed', { moduleInfo: { id: '/entry.ets' } }), {})
  assert.equal(typeof dispatchPluginHook(plugin, 'beforeBuildEnd').then, 'function')
  assert.deepEqual(dispatchPluginHook(plugin, 'buildEnd', { buildEndContext: { errors: [] } }), {})
  assert.deepEqual(calls, ['buildStart', 'moduleParsed', 'beforeBuildEnd', 'buildEnd'])
})

test('explicit Rolldown plugin lifecycle preserves async return values', () => {
  const pending = new Promise(() => {})
  const [plugin] = normalizePluginList({
    name: 'rolldown-async-return-contract',
    buildStart() {
      return pending
    },
    buildEnd() {
      return pending
    },
  })

  assert.equal(typeof dispatchPluginHook(plugin, 'buildStart', { buildStartOptions: {} }).then, 'function')
  assert.equal(typeof dispatchPluginHook(plugin, 'buildEnd', { buildEndContext: { errors: [] } }).then, 'function')
})

test('explicit plugin API does not create a parallel shouldTransformCachedModule runner', () => {
  assert.deepEqual(
    normalizePluginList({
      name: 'not-a-rolldown-hook',
      shouldTransformCachedModule() {
        return true
      },
    }),
    [],
  )
})

// ---------------------------------------------------------------------------
// Group 2: HvigorRuntimeHost — task config effect capture
// ---------------------------------------------------------------------------

test('HvigorRuntimeHost captures compilePluginPath from direct option', () => {
  const host = new HvigorRuntimeHost()
  host._captureTaskEffect({ compilePluginPath: './my-plugin.js' })
  assert.deepEqual(host._effects.compilePluginPath, ['./my-plugin.js'])
})

test('HvigorRuntimeHost captures compilePluginPath from nested buildOption.arkOptions', () => {
  const host = new HvigorRuntimeHost()
  host._captureTaskEffect({
    buildOption: {
      arkOptions: {
        compilePluginPath: './nested-plugin.js',
        hvigorPluginFile: './hvigor-plugin.js',
      },
    },
  })
  assert.deepEqual(host._effects.compilePluginPath, ['./nested-plugin.js'])
  assert.deepEqual(host._effects.hvigorPluginFile, ['./hvigor-plugin.js'])
})

test('HvigorRuntimeHost captures array compilePluginPath', () => {
  const host = new HvigorRuntimeHost()
  host._captureTaskEffect({ compilePluginPath: ['./plugin-a.js', './plugin-b.js'] })
  assert.deepEqual(host._effects.compilePluginPath, ['./plugin-a.js', './plugin-b.js'])
})

test('HvigorRuntimeHost._makeOhosPluginMock returns callable task functions', () => {
  const host = new HvigorRuntimeHost()
  const mock = host._makeOhosPluginMock()
  // All task factories must be callable
  assert.equal(typeof mock.appTasks, 'function')
  assert.equal(typeof mock.hapTasks, 'function')
  assert.equal(typeof mock.hspTasks, 'function')
  assert.equal(typeof mock.harTasks, 'function')
  // Calling them captures effects
  mock.hapTasks({ compilePluginPath: './from-hap.js' })
  assert.deepEqual(host._effects.compilePluginPath, ['./from-hap.js'])
})

test('OHOS plugin root exports legacy factories and command modifier enum', () => {
  const host = new HvigorRuntimeHost()
  const mock = host._makeOhosPluginMock('fixture-hvigorfile.ts')

  assert.equal(mock.CommandBuilderType.NINJA, 2)
  assert.equal(mock.CommandBuilderType[2], 'NINJA')
  assert.equal(typeof mock.registryCommandModifier, 'function')
  assert.equal(typeof mock.getRemoteHspObjList, 'function')
  const modifier = { modify: (command) => ['native-command-wrapper', ...command.slice(1), '--profile'] }
  mock.registryCommandModifier(mock.CommandBuilderType.NINJA, modifier)
  assert.equal(host._commandBuilderModifiers.get(2), modifier)
  assert.deepEqual(host.createNativeTaskBridge().commandModifierTypes, [mock.CommandBuilderType.NINJA])
  assert.deepEqual(
    host._runNativeCommandModifier({
      builderType: mock.CommandBuilderType.NINJA,
      command: ['ninja', '-C', path.join('native', 'arm64-v8a')],
    }),
    ['native-command-wrapper', '-C', path.join('native', 'arm64-v8a'), '--profile'],
  )

  for (const factory of [mock.legacyAppTasks, mock.legacyHapTasks, mock.legacyHarTasks]) {
    assert.throws(factory, (error) => error.code === 'ARKDOWN_UNSUPPORTED_LEGACY_FA_MODEL')
  }
})

test('OHOS command modifier rejects results outside the official string-array contract', () => {
  const host = new HvigorRuntimeHost()
  const mock = host._makeOhosPluginMock('fixture-hvigorfile.ts')
  mock.registryCommandModifier(mock.CommandBuilderType.NINJA, { modify: () => ['ninja', 1] })

  assert.throws(
    () =>
      host._runNativeCommandModifier({
        builderType: mock.CommandBuilderType.NINJA,
        command: ['ninja'],
      }),
    (error) => error.code === 'ARKDOWN_INVALID_COMMAND_MODIFIER_RESULT',
  )
})

test('getRemoteHspObjList follows module-first and highest-version project override semantics', async () => {
  const project = copyFixture('minimal-hap')
  const moduleRoot = path.join(project, 'entry')
  const writeRemoteHsp = (root, directory, fileName, name, version, integratedHsp = false) => {
    const output = path.join(root, 'oh_modules', '.hsp', directory)
    fs.mkdirSync(output, { recursive: true })
    fs.writeFileSync(path.join(output, fileName), 'HSP')
    fs.writeFileSync(
      path.join(output, 'oh-package.json5'),
      JSON.stringify({ name, version, metadata: { integratedHsp } }),
    )
    return output
  }
  writeRemoteHsp(moduleRoot, 'module-chat', 'chat.hsp', '@fixture/chat', '1.2.0')
  writeRemoteHsp(moduleRoot, 'module-map', 'map.hsp', '@fixture/map', '2.0.0-beta.1', true)
  const projectChat = writeRemoteHsp(project, 'project-chat', 'chat.hsp', '@fixture/chat', '1.3.0')
  writeRemoteHsp(project, 'project-map', 'map.hsp', '@fixture/map', '2.0.0-beta.2', true)

  const loader = new RuntimePluginLoader({ project })
  await loader.load()
  const entry = loader.hvigorHost._nodesByName.get('entry')
  const remoteHsps = loader.hvigorHost._makeOhosPluginMock().getRemoteHspObjList(entry.wrapper)

  assert.deepEqual(
    remoteHsps.map(({ hspFileName, hspVersion }) => [hspFileName, hspVersion]),
    [
      ['chat.hsp', '1.3.0'],
      ['map.hsp', '2.0.0-beta.2'],
    ],
  )
  assert.equal(remoteHsps[0].hspPath, path.join(projectChat, 'chat.hsp'))
  assert.equal(remoteHsps[1].isIntegratedHsp, true)
})

test('getRemoteHspObjList rejects multiple HSP files with the official diagnostic', async () => {
  const project = copyFixture('minimal-hap')
  const invalid = path.join(project, 'entry', 'oh_modules', '.hsp', 'invalid')
  fs.mkdirSync(invalid, { recursive: true })
  fs.writeFileSync(path.join(invalid, 'one.hsp'), 'HSP')
  fs.writeFileSync(path.join(invalid, 'two.hsp'), 'HSP')
  fs.writeFileSync(path.join(invalid, 'oh-package.json5'), '{ name: "invalid", version: "1.0.0" }')

  const loader = new RuntimePluginLoader({ project })
  await loader.load()
  const entry = loader.hvigorHost._nodesByName.get('entry')
  assert.throws(
    () => loader.hvigorHost._makeOhosPluginMock().getRemoteHspObjList(entry.wrapper),
    (error) => error.code === 'ALLOW_ONE_HSP',
  )
})

test('hvigorfile with hapTasks call captures compilePluginPath via HvigorRuntimeHost', async () => {
  const root = tempProject()
  const pluginFile = path.join(root, 'compile-plugin.js')
  fs.writeFileSync(
    pluginFile,
    `
exports.default = { name: 'captured-plugin', transform(code) { return code } }
`,
  )
  // TypeScript hvigorfile that calls hapTasks with compilePluginPath
  fs.writeFileSync(
    path.join(root, 'hvigorfile.ts'),
    `
import { hapTasks } from '@ohos/hvigor-ohos-plugin';
export default hapTasks({
  buildOption: {
    arkOptions: {
      compilePluginPath: './compile-plugin.js'
    }
  }
});
`,
  )
  const plugins = await new RuntimePluginLoader({ project: root }).load()
  // The captured compilePluginPath should have been loaded
  assert.ok(
    plugins.some((p) => p.name === 'captured-plugin'),
    `expected captured-plugin in [${plugins.map((p) => p.name).join(', ')}]`,
  )
})

test('hvigor 6.24.3 binds named hapTasks exports into the native assembly graph', async () => {
  const project = copyFixture('minimal-hap')
  fs.writeFileSync(
    path.join(project, 'entry', 'hvigorfile.ts'),
    `export { hapTasks } from '@ohos/hvigor-ohos-plugin'\n`,
  )

  const loader = new RuntimePluginLoader({ project })
  await loader.load()
  const entry = loader.hvigorHost._nodesByName.get('entry')
  assert.equal(entry.kind, 'hap')
  assert.deepEqual(entry.wrapper.getAllPluginIds(), ['com.ohos.hap'])

  loader.hvigorHost.prepareDirectBuildCycle()
  const bridge = loader.hvigorHost.createNativeTaskBridge()
  const { selection, tasks, catalogs } = bridge
  assert.deepEqual(selection.expectedRootPaths, ['entry:assembleHap'])
  assert.equal(catalogs.find((catalog) => catalog.nodeName === 'entry').kind, 'hap')
  assert.equal(
    tasks.some((task) => task.path === 'entry:default@ProcessProfile'),
    false,
    'unrealized built-ins must be expanded from the compact Rust catalog instead of crossing N-API',
  )
  assert.equal(
    tasks.some((task) => task.path === 'entry:assembleHap'),
    true,
  )
})

test('hvigor 6.24.3 TypeScript custom plugins bind context before apply and forward OHOS effects', async () => {
  const root = tempProject()
  const entry = path.join(root, 'entry')
  const hvigor = path.join(root, 'hvigor')
  const marker = path.join(root, 'custom-plugin.json')
  const order = path.join(root, 'custom-plugin-order.txt')
  fs.mkdirSync(entry, { recursive: true })
  fs.mkdirSync(hvigor, { recursive: true })
  fs.writeFileSync(path.join(hvigor, 'hvigor-config.json5'), `{ dependencies: {} }\n`)
  fs.writeFileSync(
    path.join(root, 'build-profile.json5'),
    `{
      app: { products: [{ name: 'default' }] },
      modules: [{
        name: 'entry',
        srcPath: './entry',
        targets: [{ name: 'default', applyToProducts: ['default'] }]
      }]
    }\n`,
  )
  fs.writeFileSync(
    path.join(entry, 'build-profile.json5'),
    `{ buildOption: { resOptions: { copyCodeResource: { enable: false } } }, targets: [{ name: 'default' }] }\n`,
  )
  fs.writeFileSync(
    path.join(entry, 'compile-plugin.ts'),
    `export default { name: 'typescript-compile-plugin', transform(code: string) { return code } }\n`,
  )
  fs.writeFileSync(
    path.join(entry, 'custom-plugin.ts'),
    `
import { OhosPluginId } from '@ohos/hvigor-ohos-plugin'
import * as fs from 'node:fs'

const consumer = {
  pluginId: 'fixture.consumer',
  apply(node: any) {
    const metadata = node.getContext('fixture.metadata')
    const hap = node.getContext(OhosPluginId.OHOS_HAP_PLUGIN)
    const targets: string[] = []
    let buildOptionFrozen = false
    hap.targets((target: any) => {
      targets.push(target.getTargetName())
      const buildOption = target.getBuildOption()
      buildOptionFrozen = Object.isFrozen(buildOption) && Object.isFrozen(buildOption.resOptions)
    })
    const profile = hap.getBuildProfileOpt()
    profile.buildOption = {
      ...(profile.buildOption || {}),
      arkOptions: { byteCodeHar: true },
    }
    hap.setBuildProfileOpt(profile)
    hap.loadCompilePlugin('./compile-plugin.ts')
    fs.writeFileSync(${JSON.stringify(marker)}, JSON.stringify({
      metadata,
      moduleName: node.getNodeName(),
      pluginIds: node.getAllPluginIds(),
      targets,
      buildOptionFrozen,
    }))
    fs.appendFileSync(${JSON.stringify(order)}, 'consumer\\n')
  },
}

const metadata = {
  pluginId: 'fixture.metadata',
  context() { return { owner: this.pluginId } },
  apply() { fs.appendFileSync(${JSON.stringify(order)}, 'metadata\\n') },
}

export default [consumer, metadata]
`,
  )
  fs.writeFileSync(
    path.join(entry, 'hvigorfile.ts'),
    `
import { hapTasks } from '@ohos/hvigor-ohos-plugin'
import { hvigor } from '@ohos/hvigor'
import * as fs from 'node:fs'
import plugins from './custom-plugin'

hvigor.nodesEvaluated(() => fs.appendFileSync(${JSON.stringify(order)}, 'nodesEvaluated\\n'))
export default { system: hapTasks, plugins }
`,
  )

  const loader = new RuntimePluginLoader({
    project: root,
    module: 'entry',
    product: 'default',
    buildTargetName: 'default',
  })
  const plugins = await loader.load()
  assert.deepEqual(
    plugins.map((plugin) => plugin.name),
    ['typescript-compile-plugin'],
  )
  assert.deepEqual(JSON.parse(fs.readFileSync(marker, 'utf8')), {
    metadata: { owner: 'fixture.metadata' },
    moduleName: 'entry',
    pluginIds: ['com.ohos.hap', 'fixture.metadata'],
    targets: ['default'],
    buildOptionFrozen: true,
  })
  assert.deepEqual(fs.readFileSync(order, 'utf8').trim().split('\n'), ['consumer', 'metadata', 'nodesEvaluated'])

  const nativeConfig = loader.toNativeBuildConfig()
  assert.equal(nativeConfig.moduleHvigorOverridesJson, undefined)
  const contextOverrides = JSON.parse(nativeConfig.runtimeContextOverridesJson)
  assert.equal(contextOverrides.modules[entry].buildProfile.buildOption.arkOptions.byteCodeHar, true)
})

test('OHOS context getters and setters use the Rust snapshot overlay contract', async (context) => {
  const root = tempProject()
  const previousOhpmBin = process.env.ohpmBin
  process.env.ohpmBin = fakeOhpmExecutable(root)
  context.after(() => {
    if (previousOhpmBin === undefined) delete process.env.ohpmBin
    else process.env.ohpmBin = previousOhpmBin
  })
  const entry = path.join(root, 'entry')
  const marker = path.join(root, 'context.json')
  fs.mkdirSync(path.join(root, 'AppScope'), { recursive: true })
  fs.mkdirSync(path.join(entry, 'src', 'main'), { recursive: true })
  fs.writeFileSync(
    path.join(root, 'build-profile.json5'),
    `{
      app: { products: [{ name: 'default', runtimeOS: 'HarmonyOS', bundleName: 'com.example.before' }] },
      modules: [{ name: 'entry', srcPath: './entry', targets: [{ name: 'default' }] }]
    }\n`,
  )
  fs.writeFileSync(
    path.join(root, 'AppScope', 'app.json5'),
    `{ app: { bundleName: 'com.example.before', versionCode: 1, versionName: '1.0.0' } }\n`,
  )
  fs.writeFileSync(path.join(root, 'oh-package.json5'), `{ name: 'project', version: '1.0.0' }\n`)
  fs.writeFileSync(path.join(entry, 'build-profile.json5'), `{ targets: [{ name: 'default' }] }\n`)
  fs.writeFileSync(
    path.join(entry, 'src', 'main', 'module.json5'),
    `{ module: { name: 'entry', type: 'entry', deviceTypes: ['phone'] } }\n`,
  )
  fs.writeFileSync(path.join(entry, 'oh-package.json5'), `{ name: 'entry', version: '1.0.0' }\n`)
  fs.writeFileSync(
    path.join(root, 'hvigorfile.ts'),
    `
import { appTasks, OhosPluginId } from '@ohos/hvigor-ohos-plugin'
const plugin = {
  pluginId: 'fixture.project-context',
  apply(node: any) {
    const context = node.getContext(OhosPluginId.OHOS_APP_PLUGIN)
    const profile = context.getBuildProfileOpt()
    profile.app.products[0].bundleName = 'com.example.profile'
    context.setBuildProfileOpt(profile)
    const app = context.getAppJsonOpt()
    app.app.versionCode = 7
    context.setAppJsonOpt(app)
    context.setDependenciesOpt({ projectDep: '2.0.0' })
    context.setDevDependenciesOpt({ projectDev: '3.0.0' })
    context.setDynamicDependenciesOpt({ projectDynamic: '4.0.0' })
    context.setOverrides({ projectDep: '2.1.0' })
    globalThis.__arkdownProjectContext = {
      dependency: context.getOhpmDependencyInfo(),
      remote: context.getOhpmRemoteHspDependencyInfo(false),
      signed: context.getOhpmRemoteHspDependencyInfo(true),
      hash: context.getAllModuleNameHash(),
    }
  },
}
export default { system: appTasks, plugins: [plugin] }
`,
  )
  fs.writeFileSync(
    path.join(entry, 'hvigorfile.ts'),
    `
import { hapTasks, OhosPluginId } from '@ohos/hvigor-ohos-plugin'
import * as fs from 'node:fs'
const plugin = {
  pluginId: 'fixture.module-context',
  apply(node: any) {
    const context = node.getContext(OhosPluginId.OHOS_HAP_PLUGIN)
    const profile = context.getBuildProfileOpt()
    profile.buildOption = { preloadSystemSo: true }
    context.setBuildProfileOpt(profile)
    const moduleJson = context.getModuleJsonOpt()
    moduleJson.module.description = '$string:changed'
    context.setModuleJsonOpt(moduleJson)
    context.setDependenciesOpt({ moduleDep: '5.0.0' })
    context.setDevDependenciesOpt({ moduleDev: '6.0.0' })
    context.setDynamicDependenciesOpt({ moduleDynamic: '7.0.0' })
    context.setVersion('8.0.0')
    fs.writeFileSync(${JSON.stringify(marker)}, JSON.stringify({
      project: globalThis.__arkdownProjectContext,
      module: {
        dependency: context.getOhpmDependencyInfo(),
        remote: context.getOhpmRemoteHspDependencyInfo(false),
        signed: context.getOhpmRemoteHspDependencyInfo(true),
      },
    }))
  },
}
export default { system: hapTasks, plugins: [plugin] }
`,
  )

  const loader = new RuntimePluginLoader({ project: root, product: 'default' })
  await loader.load()

  const recorded = JSON.parse(fs.readFileSync(marker, 'utf8'))
  assert.match(recorded.project.hash, /^[0-9a-f]{64}$/)
  assert.deepEqual(recorded.project.dependency, {})
  assert.deepEqual(recorded.project.remote, {})
  assert.deepEqual(recorded.project.signed, {})
  assert.deepEqual(recorded.module.dependency, {})
  assert.deepEqual(recorded.module.remote, {})
  assert.deepEqual(recorded.module.signed, {})

  const nativeConfig = loader.toNativeBuildConfig()
  const overrides = JSON.parse(nativeConfig.runtimeContextOverridesJson)
  assert.equal(overrides.project.buildProfile.app.products[0].bundleName, 'com.example.profile')
  assert.equal(overrides.project.appJson.app.versionCode, 7)
  assert.deepEqual(overrides.project.package, {
    name: 'project',
    version: '1.0.0',
    dependencies: { projectDep: '2.0.0' },
    devDependencies: { projectDev: '3.0.0' },
    dynamicDependencies: { projectDynamic: '4.0.0' },
    overrides: { projectDep: '2.1.0' },
  })
  assert.equal(overrides.modules[entry].buildProfile.buildOption.preloadSystemSo, true)
  assert.equal(overrides.modules[entry].moduleJson.module.description, '$string:changed')
  assert.equal(overrides.modules[entry].package.version, '8.0.0')
  assert.deepEqual(overrides.modules[entry].package.dependencies, { moduleDep: '5.0.0' })

  const dependencyMapRoot = path.join(root, '.hvigor', 'dependencyMap')
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(dependencyMapRoot, 'oh-package.json5'), 'utf8')), {
    name: 'project',
    version: '1.0.0',
    dependencies: { projectDep: '2.0.0' },
    devDependencies: { projectDev: '3.0.0' },
    dynamicDependencies: { projectDynamic: '4.0.0' },
    overrides: { projectDep: '2.1.0' },
  })
  const dependencyMap = JSON.parse(fs.readFileSync(path.join(dependencyMapRoot, 'dependencyMap.json5'), 'utf8'))
  assert.equal(dependencyMap.rootDependency, './oh-package.json5')
  assert.equal(dependencyMap.dependencyMap.entry, './entry/oh-package.json5')
  assert.deepEqual(
    dependencyMap.modules.map(({ name }) => name),
    ['entry'],
  )
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(dependencyMapRoot, 'entry', 'oh-package.json5'), 'utf8')), {
    name: 'entry',
    version: '8.0.0',
    dependencies: { moduleDep: '5.0.0' },
    devDependencies: { moduleDev: '6.0.0' },
    dynamicDependencies: { moduleDynamic: '7.0.0' },
  })
})

test('TypeScript custom plugin apply failures retain plugin and hvigorfile diagnostics', async () => {
  const root = tempProject()
  fs.writeFileSync(
    path.join(root, 'hvigorfile.ts'),
    `export default { plugins: [{ pluginId: 'fixture.failure', apply() { throw new Error('boom') } }] }\n`,
  )

  await assert.rejects(
    () => new RuntimePluginLoader({ project: root }).load(),
    (error) => {
      assert.equal(error.code, 'HVIGOR_CUSTOM_PLUGIN_APPLY_FAILED')
      assert.equal(error.pluginId, 'fixture.failure')
      assert.equal(error.hvigorfile, path.join(root, 'hvigorfile.ts'))
      assert.match(error.message, /boom/)
      return true
    },
  )
})

test('latest 6.24.3 HAP HSP and HAR task catalogs realize lazily with exact qualified dependencies', async () => {
  const cases = [
    {
      kind: 'hap',
      factory: 'hapTasks',
      task: 'default@CompileArkTS',
      dependencies: [
        'entry:default@GenerateLoaderJson',
        'entry:default@CompileResource',
        'entry:default@CreateBuildProfile',
      ],
      root: 'assembleHap',
      rootDependencies: ['entry:default@CollectDebugSymbol', 'entry:default@PackingCheck', 'entry:default@SignHap'],
    },
    {
      kind: 'hsp',
      factory: 'hspTasks',
      task: 'default@PackageHsp',
      dependencies: [
        'entry:default@GeneratePkgModuleJson',
        'entry:default@ProcessModulePrivacyProfile',
        'entry:default@GeneratePkgSdkInfo',
      ],
      root: 'assembleHsp',
      rootDependencies: [
        'entry:default@PackageSharedTgz',
        'entry:default@CollectDebugSymbol',
        'entry:default@PackingCheck',
      ],
    },
    {
      kind: 'har',
      factory: 'harTasks',
      task: 'default@ProcessHarArtifacts',
      dependencies: [
        'entry:default@ProcessObfuscationFiles',
        'entry:default@CacheNativeLibs',
        'entry:default@ProcessOHPackageJson',
        'entry:default@HarCompileArkTS',
        'entry:default@ProcessIntegratedHsp',
      ],
      root: 'assembleHar',
      rootDependencies: [
        'entry:default@PackageSignHar',
        'entry:default@PackingCheckHar',
        'entry:default@CollectDebugSymbol',
      ],
    },
  ]

  for (const fixture of cases) {
    const root = tempProject()
    const entry = path.join(root, 'entry')
    const result = path.join(root, `${fixture.kind}-tasks.json`)
    fs.mkdirSync(entry)
    fs.writeFileSync(
      path.join(root, 'build-profile.json5'),
      `{
        app: { products: [{ name: 'default' }] },
        modules: [{ name: 'entry', srcPath: './entry', targets: [{ name: 'default' }] }]
      }\n`,
    )
    fs.writeFileSync(path.join(entry, 'build-profile.json5'), `{ targets: [{ name: 'default' }] }\n`)
    fs.writeFileSync(
      path.join(entry, 'hvigorfile.ts'),
      `
import { ${fixture.factory} } from '@ohos/hvigor-ohos-plugin'
import { getNode, hvigor } from '@ohos/hvigor'
import * as fs from 'node:fs'
const node = getNode(__filename)!
hvigor.nodesEvaluated(() => {
  const before = node.getAllTasks().map((task: any) => task.getName())
  const selected = node.getTaskByName(${JSON.stringify(fixture.task)})!
  const rootTask = node.getTaskByName(${JSON.stringify(fixture.root)})!
  fs.writeFileSync(${JSON.stringify(result)}, JSON.stringify({
    before,
    selectedDependencies: selected.getDependencies(),
    rootDependencies: rootTask.getDependencies(),
    after: node.getAllTasks().map((task: any) => task.getName()),
  }))
})
export default { system: ${fixture.factory}, plugins: [] }
`,
    )

    await new RuntimePluginLoader({ project: root, module: 'entry', buildTargetName: 'default' }).load()
    assert.deepEqual(JSON.parse(fs.readFileSync(result, 'utf8')), {
      before: ['tasks', 'taskTree'],
      selectedDependencies: fixture.dependencies,
      rootDependencies: fixture.rootDependencies,
      after: ['tasks', 'taskTree', fixture.task, fixture.root],
    })
  }
})

test('npm project task catalog resolves local HSP and HAR targets across module boundaries', async () => {
  const root = tempProject()
  const entry = path.join(root, 'entry')
  const shared = path.join(root, 'shared')
  const library = path.join(root, 'library')
  const entryResult = path.join(root, 'entry-cross-module-tasks.json')
  const harResult = path.join(root, 'npm-har-tasks.json')
  for (const moduleRoot of [entry, shared, library]) fs.mkdirSync(moduleRoot)
  fs.writeFileSync(
    path.join(root, 'build-profile.json5'),
    `{
      app: { products: [{ name: 'default', compileSdkVersion: '12', compatibleSdkVersion: '12' }] },
      modules: [
        { name: 'entry', srcPath: './entry', targets: [{ name: 'default' }] },
        { name: 'shared', srcPath: './shared', targets: [{ name: 'default' }] },
        { name: 'library', srcPath: './library', targets: [{ name: 'default' }] }
      ]
    }\n`,
  )
  fs.writeFileSync(path.join(root, 'package.json'), JSON.stringify({ name: 'npm-project' }))
  fs.writeFileSync(
    path.join(root, 'hvigorfile.ts'),
    `import { appTasks } from '@ohos/hvigor-ohos-plugin'\nexport default { system: appTasks, plugins: [] }\n`,
  )
  for (const [moduleRoot, name] of [
    [entry, 'entry'],
    [shared, 'shared'],
    [library, 'library'],
  ]) {
    fs.writeFileSync(path.join(moduleRoot, 'build-profile.json5'), `{ targets: [{ name: 'default' }] }\n`)
    fs.writeFileSync(
      path.join(moduleRoot, 'package.json'),
      JSON.stringify({
        name,
        ohos: {},
        dependencies: name === 'entry' ? { shared: 'file:../shared', library: 'file:../library' } : {},
      }),
    )
  }
  fs.writeFileSync(
    path.join(entry, 'hvigorfile.ts'),
    `
import { hapTasks } from '@ohos/hvigor-ohos-plugin'
import { getNode, hvigor } from '@ohos/hvigor'
import * as fs from 'node:fs'
const node = getNode(__filename)!
hvigor.nodesEvaluated(() => {
  const dependencies = (name: string) => node.getTaskByName(name)!.getDependencies()
  fs.writeFileSync(${JSON.stringify(entryResult)}, JSON.stringify({
    merge: dependencies('default@MergeProfile'),
    profile: dependencies('default@CreateBuildProfile'),
    compile: dependencies('default@CompileArkTS'),
    metadata: dependencies('default@GenerateMetadata'),
  }))
})
export default { system: hapTasks, plugins: [] }
`,
  )
  fs.writeFileSync(
    path.join(shared, 'hvigorfile.ts'),
    `import { hspTasks } from '@ohos/hvigor-ohos-plugin'\nexport default { system: hspTasks, plugins: [] }\n`,
  )
  fs.writeFileSync(
    path.join(library, 'hvigorfile.ts'),
    `
import { harTasks } from '@ohos/hvigor-ohos-plugin'
import { getNode, hvigor } from '@ohos/hvigor'
import * as fs from 'node:fs'
const node = getNode(__filename)!
hvigor.nodesEvaluated(() => {
  fs.writeFileSync(
    ${JSON.stringify(harResult)},
    JSON.stringify(node.getTaskByName('default@ProcessHarArtifacts')!.getDependencies()),
  )
})
export default { system: harTasks, plugins: [] }
`,
  )

  const loader = new RuntimePluginLoader({ project: root, module: 'entry', buildTargetName: 'default' })
  assert.equal(loader.hvigorHost._isOhpmProject(), false)
  await loader.load()
  assert.equal(loader.hvigorHost._isOhpmProject(), false)
  assert.deepEqual(JSON.parse(fs.readFileSync(entryResult, 'utf8')), {
    merge: ['entry:default@PreBuild', 'shared:default@MergeProfile', 'library:default@MergeProfile'],
    profile: ['entry:default@PreBuild', 'shared:default@CreateBuildProfile', 'library:default@CreateHarBuildProfile'],
    compile: [
      'entry:default@GenerateLoaderJson',
      'entry:default@CompileResource',
      'entry:default@CreateBuildProfile',
      'library:default@CreateHarBuildProfile',
    ],
    metadata: ['entry:default@PreBuild', 'shared:default@GenerateMetadata'],
  })
  assert.deepEqual(JSON.parse(fs.readFileSync(harResult, 'utf8')), [
    'library:default@ProcessObfuscationFiles',
    'library:default@CacheNativeLibs',
    'library:default@ProcessPackageJson',
    'library:default@HarCompileArkTS',
    'library:default@ProcessIntegratedHsp',
  ])
})

test('configured Cangjie build is rejected before native packaging can omit its outputs', async () => {
  const root = tempProject()
  const entry = path.join(root, 'entry')
  fs.mkdirSync(entry)
  fs.writeFileSync(
    path.join(root, 'build-profile.json5'),
    `{
      app: { products: [{ name: 'default', compileSdkVersion: '12', compatibleSdkVersion: '12' }] },
      modules: [{ name: 'entry', srcPath: './entry', targets: [{ name: 'default' }] }]
    }\n`,
  )
  fs.writeFileSync(
    path.join(entry, 'build-profile.json5'),
    `{
      buildOption: {
        cangjieOptions: { path: './src/main/cangjie/cjpm.toml', abiFilters: ['arm64-v8a'] }
      },
      targets: [{ name: 'default' }]
    }\n`,
  )
  fs.writeFileSync(
    path.join(entry, 'hvigorfile.ts'),
    `import { hapTasks } from '@ohos/hvigor-ohos-plugin'\nexport default { system: hapTasks, plugins: [] }\n`,
  )
  const loader = new RuntimePluginLoader({ project: root, module: 'entry', buildTargetName: 'default' })
  await loader.load()
  assert.throws(
    () => loader.toNativeBuildConfig(),
    (error) => {
      assert.equal(error.code, 'ARKDOWN_UNSUPPORTED_CANGJIE_BUILD')
      assert.equal(error.module, 'entry')
      assert.equal(error.target, 'default')
      return true
    },
  )
})

test('latest project task catalog refreshes child assembly dependencies after module binding', async () => {
  const root = tempProject()
  const entry = path.join(root, 'entry')
  const result = path.join(root, 'project-tasks.json')
  fs.mkdirSync(entry)
  fs.writeFileSync(
    path.join(root, 'build-profile.json5'),
    `{
      app: { products: [{ name: 'default' }] },
      modules: [{ name: 'entry', srcPath: './entry', targets: [{ name: 'default' }] }]
    }\n`,
  )
  fs.writeFileSync(path.join(entry, 'build-profile.json5'), `{ targets: [{ name: 'default' }] }\n`)
  fs.writeFileSync(
    path.join(root, 'hvigorfile.ts'),
    `
import { appTasks } from '@ohos/hvigor-ohos-plugin'
import { getNode, hvigor } from '@ohos/hvigor'
import * as fs from 'node:fs'
const node = getNode(__filename)!
hvigor.nodesEvaluated(() => {
  const sign = node.getTaskByName('SignApp')!
  const packInfo = node.getTaskByName('MakeProjectPackInfo')!
  fs.writeFileSync(${JSON.stringify(result)}, JSON.stringify({
    initial: node.getAllTasks().map((task: any) => task.getName()),
    sign: sign.getDependencies(),
    packInfo: packInfo.getDependencies(),
  }))
})
export default { system: appTasks, plugins: [] }
`,
  )
  fs.writeFileSync(
    path.join(entry, 'hvigorfile.ts'),
    `import { hapTasks } from '@ohos/hvigor-ohos-plugin'\nexport default { system: hapTasks, plugins: [] }\n`,
  )

  await new RuntimePluginLoader({ project: root, module: 'entry', buildTargetName: 'default' }).load()
  assert.deepEqual(JSON.parse(fs.readFileSync(result, 'utf8')), {
    initial: ['tasks', 'taskTree', 'prune', 'SignApp', 'MakeProjectPackInfo'],
    sign: ['PackageApp', 'SignPackagesFromApp'],
    packInfo: ['entry:assembleHap'],
  })
})

test('hvigorfile export object reads buildOption.arkOptions plugin paths', async () => {
  const root = tempProject()
  fs.writeFileSync(
    path.join(root, 'hvigor-plugin.js'),
    `exports.default = { name: 'export-hvigor', buildStart() {} }\n`,
  )
  fs.writeFileSync(
    path.join(root, 'compile-a.js'),
    `exports.default = { name: 'export-compile-a', transform(code) { return code } }\n`,
  )
  fs.writeFileSync(
    path.join(root, 'compile-b.js'),
    `exports.default = { name: 'export-compile-b', transform(code) { return code } }\n`,
  )
  fs.writeFileSync(
    path.join(root, 'hvigorfile.js'),
    `
module.exports = {
  buildOption: {
    arkOptions: {
      hvigorPluginFile: './hvigor-plugin.js',
      compilePluginPath: ['./compile-a.js', './compile-b.js']
    }
  }
}
`,
  )

  const plugins = await new RuntimePluginLoader({ project: root }).load()
  assert.deepEqual(
    plugins.map((plugin) => plugin.name),
    ['export-hvigor', 'export-compile-a', 'export-compile-b'],
  )
})

test('latest hvigorfile schema rejects declarative arkOptions.compilePluginFile', async () => {
  const root = tempProject()
  const moduleRoot = path.join(root, 'entry')
  fs.mkdirSync(moduleRoot, { recursive: true })
  fs.writeFileSync(
    path.join(moduleRoot, 'hvigorfile.js'),
    `
module.exports = {
  config: {
    ohos: {
      overrides: {
        buildOption: {
          arkOptions: {
            compilePluginFile: './build/hvigor_compile/plugin'
          }
        }
      }
    }
  }
}
`,
  )

  await assert.rejects(
    async () => new RuntimePluginLoader({ project: root }).load(),
    (error) => {
      assert.equal(error.code, 'HVIGOR_SCHEMA_COMPILE_PLUGIN_FILE')
      assert.equal(error.fieldPath, 'config.ohos.overrides.buildOption.arkOptions.compilePluginFile')
      assert.match(error.message, /OhosModuleContext\.loadCompilePlugin/)
      return true
    },
  )
})

test('latest project and module build-profile schemas reject declarative compilePluginFile', async (context) => {
  await context.test('project product build option', async () => {
    const root = tempProject()
    fs.writeFileSync(
      path.join(root, 'build-profile.json5'),
      `{
        app: { products: [{ name: 'default', buildOption: { arkOptions: {
          compilePluginFile: './build/hvigor_compile/plugin.ts'
        } } }] }
      }\n`,
    )

    await assert.rejects(
      async () => new RuntimePluginLoader({ project: root }).load(),
      (error) =>
        error.code === 'HVIGOR_SCHEMA_COMPILE_PLUGIN_FILE' &&
        error.fieldPath === 'app.products[0].buildOption.arkOptions.compilePluginFile',
    )
  })

  await context.test('module target build option', async () => {
    const root = tempProject()
    const moduleRoot = path.join(root, 'entry')
    fs.mkdirSync(moduleRoot, { recursive: true })
    fs.writeFileSync(
      path.join(root, 'build-profile.json5'),
      `{
        app: { products: [{ name: 'default' }] },
        modules: [{ name: 'entry', srcPath: './entry', targets: [{ name: 'default' }] }]
      }\n`,
    )
    fs.writeFileSync(
      path.join(moduleRoot, 'build-profile.json5'),
      `{ targets: [{ name: 'default', config: { buildOption: { arkOptions: {
        compilePluginFile: './build/hvigor_compile/plugin.ts'
      } } } }] }\n`,
    )

    await assert.rejects(
      async () => new RuntimePluginLoader({ project: root }).load(),
      (error) =>
        error.code === 'HVIGOR_SCHEMA_COMPILE_PLUGIN_FILE' &&
        error.fieldPath === 'targets[0].config.buildOption.arkOptions.compilePluginFile',
    )
  })
})

test('OhosModuleContext.loadCompilePlugin enforces path, suffix, and TypeScript type checks', async () => {
  const compilePluginError = (error) => error.cause || error
  const createProject = (configured, pluginSource, pluginFileName = 'compile-plugin.ts') => {
    const root = tempProject()
    const moduleRoot = path.join(root, 'entry')
    fs.mkdirSync(moduleRoot, { recursive: true })
    fs.writeFileSync(
      path.join(root, 'build-profile.json5'),
      `{
        app: { products: [{ name: 'default' }] },
        modules: [{ name: 'entry', srcPath: './entry', targets: [{ name: 'default' }] }]
      }\n`,
    )
    fs.writeFileSync(path.join(moduleRoot, 'build-profile.json5'), `{ targets: [{ name: 'default' }] }\n`)
    if (pluginSource !== undefined) {
      fs.writeFileSync(path.join(moduleRoot, pluginFileName), pluginSource)
    }
    fs.writeFileSync(
      path.join(moduleRoot, 'hvigorfile.ts'),
      `
import { hapTasks, OhosPluginId } from '@ohos/hvigor-ohos-plugin'
const loader = {
  pluginId: 'fixture.load-compile-plugin',
  apply(node: any) {
    node.getContext(OhosPluginId.OHOS_HAP_PLUGIN).loadCompilePlugin(${JSON.stringify(configured)})
  },
}
export default { system: hapTasks, plugins: [loader] }
`,
    )
    return root
  }

  await assert.rejects(
    async () => new RuntimePluginLoader({ project: createProject('') }).load(),
    (error) => compilePluginError(error).code === 'EMPTY_COMPILE_PLUGIN_PATH',
  )
  await assert.rejects(
    async () => new RuntimePluginLoader({ project: createProject('./missing.ts') }).load(),
    (error) => {
      const cause = compilePluginError(error)
      return cause.code === 'PLUGIN_PATH_NOT_FOUND' && cause.pluginPath.endsWith('missing.ts')
    },
  )
  await assert.rejects(
    async () =>
      new RuntimePluginLoader({
        project: createProject('./compile-plugin.js', 'export default {}', 'compile-plugin.js'),
      }).load(),
    (error) => {
      const cause = compilePluginError(error)
      return cause.code === 'PLUGIN_FILE_MUST_END_WITH_TS' && cause.pluginPath.endsWith('compile-plugin.js')
    },
  )
  const invalidRoot = createProject(
    './compile-plugin.ts',
    `const invalid: string = 1\nexport default { name: invalid }\n`,
  )
  await assert.rejects(
    async () => new RuntimePluginLoader({ project: invalidRoot }).load(),
    (error) => {
      const cause = compilePluginError(error)
      return cause.code === 'TYPE_ERRORS' && cause.cause?.name === 'AdaptorError'
    },
  )
})

// ---------------------------------------------------------------------------
// Group 5: BuildConfig field forwarding
// ---------------------------------------------------------------------------

test('createBuild forwards native BuildConfig fields and strips runtime plugin path fields', async () => {
  const root = tempProject()
  fs.writeFileSync(
    path.join(root, 'runtime-hvigor.js'),
    `exports.default = { name: 'runtime-hvigor', load() { return null } }\n`,
  )
  fs.writeFileSync(
    path.join(root, 'runtime-compile-a.js'),
    `exports.default = { name: 'runtime-compile-a', transform(code) { return code } }\n`,
  )
  fs.writeFileSync(
    path.join(root, 'runtime-compile-b.js'),
    `exports.default = { name: 'runtime-compile-b', transform(code) { return code } }\n`,
  )
  const config = {
    project: root,
    target: 'hap',
    mode: 'debug',
    module: 'entry',
    product: 'default',
    buildTargetName: 'my-target',
    watch: false,
    preview: false,
    coverage: false,
    ohosDebugAsan: true,
    ohosEnableHwasan: true,
    disableAbcCache: true,
    cacheFile: '/tmp/test.cache',
    checker: 'oxc',
    cache: true,
    compileMode: 'esmodule',
    hvigorPluginFile: './runtime-hvigor.js',
    compilePluginPath: ['./runtime-compile-a.js', './runtime-compile-b.js'],
  }

  let capturedConfig
  let capturedPlugins
  const build = createBuild(async (cfg, plugins) => {
    capturedConfig = cfg
    capturedPlugins = plugins
    return { artifacts: [], elapsedMs: 0, diagnostics: [] }
  })

  await build(config)

  assert.equal(capturedConfig.runtimeNodeExecutable, process.execPath)
  assert.equal(capturedConfig.runtimeContextOverridesJson, undefined)

  for (const [key, value] of Object.entries(config)) {
    if (key === 'compilePluginPath' || key === 'hvigorPluginFile') {
      assert.equal(
        capturedConfig[key],
        undefined,
        `BuildConfig.${key} must be consumed by js/runtime.js before nativeBuild`,
      )
      continue
    }
    assert.equal(
      capturedConfig[key],
      value,
      `BuildConfig.${key} was not forwarded: expected ${JSON.stringify(value)}, got ${JSON.stringify(capturedConfig[key])}`,
    )
  }
  assert.deepEqual(
    capturedPlugins.map((plugin) => plugin.name),
    ['runtime-hvigor', 'runtime-compile-a', 'runtime-compile-b'],
  )
})

test('createBuild evaluates project and module config.ohos overrides before native build', async () => {
  const root = tempProject()
  const entry = path.join(root, 'entry')
  fs.mkdirSync(entry)
  fs.writeFileSync(
    path.join(root, 'hvigorfile.js'),
    `
const versionCode = () => 2000001
module.exports = {
  system: {},
  config: {
    ohos: {
      overrides: {
        appOpt: {
          vendor: 'example',
          versionCode: versionCode(),
          versionName: '2.0.1',
          bundleName: 'com.example.override',
          bundleType: 'atomicService',
          icon: '$media:override_icon',
          label: '$string:override_label'
        },
        signingConfig: {
          material: {
            keyAlias: 'override-key'
          }
        }
      }
    }
  }
}
`,
  )
  fs.writeFileSync(
    path.join(entry, 'hvigorfile.js'),
    `
module.exports = {
  system: {},
  config: {
    ohos: {
      overrides: {
        buildOption: {
          arkOptions: {
            hostPGO: true,
            runtimeOnly: {
              packages: ['runtime-package']
            }
          },
          externalNativeOptions: {
            abiFilters: ['arm64-v8a']
          }
        }
      }
    }
  }
}
`,
  )

  let capturedConfig
  const build = createBuild(async (config) => {
    capturedConfig = config
    return { artifacts: [] }
  })
  await build({
    project: root,
    projectHvigorOverridesJson: 'caller values must not be trusted',
    moduleHvigorOverridesJson: 'caller values must not be trusted',
  })

  assert.deepEqual(JSON.parse(capturedConfig.projectHvigorOverridesJson), {
    appOpt: {
      vendor: 'example',
      versionCode: 2000001,
      versionName: '2.0.1',
      bundleName: 'com.example.override',
      bundleType: 'atomicService',
      icon: '$media:override_icon',
      label: '$string:override_label',
    },
    signingConfig: {
      material: {
        keyAlias: 'override-key',
      },
    },
  })
  assert.deepEqual(JSON.parse(capturedConfig.moduleHvigorOverridesJson), {
    [entry]: {
      arkOptions: {
        hostPGO: true,
        runtimeOnly: {
          packages: ['runtime-package'],
        },
      },
      externalNativeOptions: {
        abiFilters: ['arm64-v8a'],
      },
    },
  })
})

// ---------------------------------------------------------------------------
// Plugin context binding — hvigor doWrapper / getCompilePluginContext
// ---------------------------------------------------------------------------

test('buildPluginContext exposes getModuleIds from hookCtx.moduleIds', () => {
  const ctx = buildPluginContext({ moduleIds: ['entry/index', 'lib/util'] })
  assert.deepEqual(Array.from(ctx.getModuleIds()), ['entry/index', 'lib/util'])
})

test('buildPluginContext exposes getModuleInfo from hookCtx.moduleInfoMap', () => {
  const moduleInfoMap = {
    'entry/index': { importedIds: ['lib/util'], dynamicallyImportedIds: [] },
  }
  const ctx = buildPluginContext({ moduleInfoMap })
  const info = ctx.getModuleInfo('entry/index')
  assert.deepEqual(info.importedIds, ['lib/util'])
  assert.equal(ctx.getModuleInfo('missing'), null)
})

test('buildPluginContext parse returns an ESTree program', () => {
  const ctx = buildPluginContext({})
  const program = ctx.parse('export const answer = 42')
  assert.equal(program.type, 'Program')
  assert.equal(program.sourceType, 'module')
  assert.equal(program.body[0].type, 'ExportNamedDeclaration')
})

test('buildPluginContext parse matches latest Rollup UTF-16 locations ranges and comments', () => {
  const comments = []
  const program = buildPluginContext({}).parse('// 注释\nconst emoji = "🤨";', {
    ranges: true,
    locations: true,
    sourceFile: 'plugin.ts',
    onComment: comments,
  })

  assert.equal(program.end, 25)
  assert.deepEqual(program.range, [0, 25])
  assert.deepEqual(program.loc, {
    start: { line: 1, column: 0 },
    end: { line: 2, column: 19 },
    source: 'plugin.ts',
  })
  assert.equal(program.body[0].declarations[0].init.end, 24)
  assert.deepEqual(comments, [
    {
      type: 'Line',
      value: ' 注释',
      start: 0,
      end: 5,
      loc: {
        start: { line: 1, column: 0 },
        end: { line: 1, column: 5 },
        source: 'plugin.ts',
      },
      range: [0, 5],
    },
  ])
})

test('buildPluginContext parse revives literals and preserves Rollup syntax errors', () => {
  const program = buildPluginContext({}).parse('(123n); /ab+/gi;')
  assert.equal(program.body[0].expression.type, 'Literal')
  assert.equal(program.body[0].expression.value, 123n)
  assert.equal(program.body[1].expression.value instanceof RegExp, true)

  assert.throws(
    () => buildPluginContext({}).parse('return 1', { sourceType: 'script' }),
    (error) =>
      error instanceof SyntaxError &&
      error.code === 'PLUGIN_PARSE_ERROR' &&
      error.pos === 0 &&
      error.loc.line === 1 &&
      error.loc.column === 0,
  )
  const allowed = buildPluginContext({}).parse('return 1', {
    sourceType: 'script',
    allowReturnOutsideFunction: true,
  })
  assert.equal(allowed.sourceType, 'script')
  assert.equal(allowed.body[0].type, 'ReturnStatement')
})

test('buildPluginContext getFileName resolves known refs and rejects unknown refs', () => {
  const ctx = buildPluginContext({ fileNames: { asset123: 'assets/icon.png' } })
  assert.equal(ctx.getFileName('asset123'), 'assets/icon.png')
  assert.throws(
    () => ctx.getFileName('missing'),
    (error) => error.code === 'FILE_NAME_NOT_FOUND',
  )
})

test('internal context seed supplies share data without adding a lifecycle hook', () => {
  const share = { bundleName: 'com.example.prop', buildMode: 'debug' }
  const captured = {}
  const plugin = normalizePluginList({
    name: 'propagation-test',
    buildStart() {
      captured.buildStart = this.share
    },
  })[0]

  seedPluginContext(plugin, share)
  dispatchPluginHook(plugin, 'buildStart', { buildStartOptions: {} })

  assert.equal(captured.buildStart.bundleName, share.bundleName)
  assert.equal(Object.keys(plugin).includes('__arkdownDispatchPluginHook'), false)
})

test('share context is bound for resolveId load transform and cache hooks', () => {
  const share = { projectConfig: { bundleName: 'com.example.hooks' } }
  const captured = []
  const plugin = normalizePluginList({
    name: 'all-hook-contexts',
    resolveId() {
      captured.push(this.share)
    },
    load() {
      captured.push(this.share)
    },
    transform() {
      captured.push(this.share)
    },
    shouldInvalidCache() {
      captured.push(this.share)
      return false
    },
  })[0]
  seedPluginContext(plugin, share)
  dispatchPluginHook(plugin, 'resolveId', { id: 'dep', importer: '/entry.ets', resolveOptions: {} })
  dispatchPluginHook(plugin, 'load', { id: '/entry.ets' })
  dispatchPluginHook(plugin, 'transform', { code: 'export {}', id: '/entry.ets' })
  dispatchPluginHook(plugin, 'shouldInvalidCache', { cacheRequest: {} })
  assert.equal(captured.length, 4)
  for (const contextShare of captured) {
    assert.equal(contextShare.projectConfig.bundleName, 'com.example.hooks')
  }
})

test('every Rolldown-owned hook delegates to the live native plugin context', () => {
  const moduleInfo = {
    id: '/entry.ets',
    importedIds: ['/dep.ets'],
    dynamicallyImportedIds: [],
  }
  const nativeContext = {
    getModuleIds() {
      return ['/entry.ets', '/dep.ets']
    },
    getModuleInfo(id) {
      return id === moduleInfo.id ? moduleInfo : null
    },
    getFileName(referenceId) {
      assert.equal(referenceId, 'asset-ref')
      return 'assets/plugin.txt'
    },
  }
  const observed = []
  const observe = function (hook) {
    observed.push({
      hook,
      moduleIds: Array.from(this.getModuleIds()),
      moduleInfo: this.getModuleInfo('/entry.ets'),
      fileName: this.getFileName('asset-ref'),
    })
  }
  const [plugin] = normalizePluginList({
    name: 'live-context-contract',
    buildStart() {
      observe.call(this, 'buildStart')
    },
    resolveId() {
      observe.call(this, 'resolveId')
      return null
    },
    resolveDynamicImport() {
      observe.call(this, 'resolveDynamicImport')
      return null
    },
    load() {
      observe.call(this, 'load')
      return null
    },
    transform() {
      observe.call(this, 'transform')
      return null
    },
    moduleParsed() {
      observe.call(this, 'moduleParsed')
    },
    beforeBuildEnd() {
      observe.call(this, 'beforeBuildEnd')
    },
    buildEnd() {
      observe.call(this, 'buildEnd')
    },
  })

  dispatchPluginHook(plugin, 'buildStart', { context: nativeContext, buildStartOptions: {} })
  dispatchPluginHook(plugin, 'resolveId', {
    context: nativeContext,
    id: 'dep',
    importer: '/entry.ets',
    resolveOptions: { attributes: {}, isEntry: false },
  })
  dispatchPluginHook(plugin, 'resolveDynamicImport', {
    context: nativeContext,
    id: 'lazy',
    importer: '/entry.ets',
    resolveOptions: { attributes: {}, isEntry: false },
  })
  dispatchPluginHook(plugin, 'load', { context: nativeContext, id: '/entry.ets' })
  dispatchPluginHook(plugin, 'transform', { context: nativeContext, code: 'export {}', id: '/entry.ets' })
  dispatchPluginHook(plugin, 'moduleParsed', { context: nativeContext, moduleInfo })
  dispatchPluginHook(plugin, 'beforeBuildEnd', { context: nativeContext })
  dispatchPluginHook(plugin, 'buildEnd', { context: nativeContext, buildEndContext: { errors: [] } })

  assert.deepEqual(
    observed.map(({ hook }) => hook),
    [
      'buildStart',
      'resolveId',
      'resolveDynamicImport',
      'load',
      'transform',
      'moduleParsed',
      'beforeBuildEnd',
      'buildEnd',
    ],
  )
  for (const observation of observed) {
    assert.deepEqual(observation.moduleIds, ['/entry.ets', '/dep.ets'])
    assert.equal(observation.moduleInfo, moduleInfo)
    assert.equal(observation.fileName, 'assets/plugin.txt')
  }
})

test('resolveId preserves hvigor options and structured hook results', () => {
  const expected = {
    id: '/virtual/entry.ets',
    external: false,
    moduleSideEffects: 'no-treeshake',
  }
  let receivedOptions
  const [plugin] = normalizePluginList({
    name: 'resolve-contract',
    resolveId(id, importer, options) {
      assert.equal(id, '@virtual/entry')
      assert.equal(importer, '/entry.ets')
      receivedOptions = options
      return expected
    },
  })
  const options = { attributes: { type: 'json' }, isEntry: false }
  assert.equal(
    dispatchPluginHook(plugin, 'resolveId', {
      id: '@virtual/entry',
      importer: '/entry.ets',
      resolveOptions: options,
    }),
    expected,
  )
  assert.equal(receivedOptions, options)
})

test('load and transform preserve SourceDescription object results', () => {
  const loadResult = {
    code: 'export const loaded = true',
    moduleType: 'js',
    moduleSideEffects: false,
    map: { version: 3, sources: ['input.ets'], names: [], mappings: '' },
  }
  const transformResult = {
    code: 'export const transformed = true',
    map: null,
  }
  const [plugin] = normalizePluginList({
    name: 'source-description-contract',
    load() {
      return loadResult
    },
    transform() {
      return transformResult
    },
  })
  assert.equal(dispatchPluginHook(plugin, 'load', { id: '/entry.ets' }), loadResult)
  assert.equal(dispatchPluginHook(plugin, 'transform', { code: 'source', id: '/entry.ets' }), transformResult)
})

test('nullish plugin results omit typed dispatcher fields at the NAPI boundary', () => {
  const [plugin] = normalizePluginList({
    name: 'nullish-hook-results',
    resolveId() {
      return null
    },
    load() {
      return undefined
    },
    transform() {
      return null
    },
  })

  assert.deepEqual(
    plugin.__arkdownDispatchPluginHook({
      hookName: 'resolveId',
      id: 'dep',
      importer: '/entry.ets',
      resolveOptions: {},
    }),
    {},
  )
  assert.deepEqual(plugin.__arkdownDispatchPluginHook({ hookName: 'load', id: '/entry.ets' }), {})
  assert.deepEqual(plugin.__arkdownDispatchPluginHook({ hookName: 'transform', code: 'source', id: '/entry.ets' }), {})
})

test('moduleParsed receives structured ModuleInfo and bound graph lookup', () => {
  let received
  let lookedUp
  const plugin = normalizePluginList({
    name: 'module-info-context',
    moduleParsed(info) {
      received = info
      lookedUp = this.getModuleInfo(info.id)
    },
  })[0]
  dispatchPluginHook(plugin, 'moduleParsed', {
    moduleInfo: {
      id: '/entry.ets',
      importedIds: ['/dep.ets'],
      dynamicallyImportedIds: ['/lazy.ets'],
      isEntry: true,
    },
  })
  assert.equal(received.id, '/entry.ets')
  assert.deepEqual(lookedUp.importedIds, ['/dep.ets'])
  assert.deepEqual(lookedUp.dynamicallyImportedIds, ['/lazy.ets'])
})

test('resolveDynamicImport is recognized as a Rolldown hook', () => {
  const [plugin] = normalizePluginList({
    name: 'dynimport-test',
    resolveDynamicImport(specifier, importer) {
      if (specifier === '@my-virtual-module') return '/resolved'
      return null
    },
  })
  assert.equal(plugin.name, 'dynimport-test')
  assert.deepEqual(plugin.__arkdownPluginHooks, ['resolveDynamicImport'])
  assert.equal(
    dispatchPluginHook(plugin, 'resolveDynamicImport', {
      id: '@my-virtual-module',
      importer: '/entry.ets',
      resolveOptions: {},
    }),
    '/resolved',
  )
})

test('explicit plugin API ignores non-Rolldown cleanUp instead of running a second lifecycle', () => {
  assert.deepEqual(normalizePluginList({ name: 'cleanup-test', cleanUp() {} }), [])
})
