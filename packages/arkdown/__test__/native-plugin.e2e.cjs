'use strict'

const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')

const { build } = require('..')

const workspaceRoot = path.resolve(__dirname, '..', '..', '..')
const enabled = process.env.ARKDOWN_NATIVE_PLUGIN_E2E === '1'

function prepareCompilePluginProject(project) {
  fs.cpSync(path.join(workspaceRoot, 'tests', 'fixtures', 'minimal-hap'), project, { recursive: true })
  const compileRoot = path.join(project, 'entry', 'compile-plugins')
  fs.mkdirSync(compileRoot, { recursive: true })
  fs.copyFileSync(
    path.join(workspaceRoot, 'tests', 'fixtures', 'compile-plugins', 'lifecycle.ts'),
    path.join(compileRoot, 'lifecycle.ts'),
  )
}

test('TypeScript compile plugin executes its complete source-module lifecycle', { skip: !enabled }, async (context) => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-native-plugin-'))
  const project = path.join(tempRoot, 'minimal-hap')
  const tracePath = path.join(tempRoot, 'plugin-trace.json')
  context.after(() => fs.rmSync(tempRoot, { recursive: true, force: true }))
  prepareCompilePluginProject(project)

  const priorTrace = process.env.ARKDOWN_PLUGIN_TRACE
  const priorInvalidate = process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE
  process.env.ARKDOWN_PLUGIN_TRACE = tracePath
  delete process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE
  context.after(() => {
    if (priorTrace === undefined) delete process.env.ARKDOWN_PLUGIN_TRACE
    else process.env.ARKDOWN_PLUGIN_TRACE = priorTrace
    if (priorInvalidate === undefined) delete process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE
    else process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE = priorInvalidate
  })

  const config = {
    project,
    target: 'hap',
    mode: 'debug',
    analyze: 'verbose',
    module: 'entry',
    product: 'default',
    buildTargetName: 'default',
    compilePluginPath: 'entry/compile-plugins/lifecycle.ts',
    cache: true,
  }
  const first = await build(config)
  assert.equal(
    first.artifacts.some((artifact) => artifact.endsWith('.hap')),
    true,
  )

  const firstTrace = JSON.parse(fs.readFileSync(tracePath, 'utf8'))
  assert.deepEqual(firstTrace.events, [
    'beforeBuild',
    'buildStart',
    'afterBuildStart',
    'beforeBuildEnd',
    'buildEnd',
    'afterBuildEnd',
    'bundleEnd',
    'cleanUp',
  ])
  assert.equal(firstTrace.buildStartArgumentCount, 0)
  assert.equal(firstTrace.beforeBuildEndArgumentCount, 1)
  assert.equal(firstTrace.buildEndArgumentCount, 1)
  assert.equal(firstTrace.buildEndError, undefined)
  assert.equal(firstTrace.resolveIdCount > 0, true)
  assert.equal(firstTrace.loadCount > 0, true)
  assert.equal(firstTrace.transformCount > 0, true)
  assert.equal(firstTrace.moduleParsedCount > 0, true)
  assert.equal(firstTrace.moduleInfoCount, firstTrace.moduleIds.length)
  assert.equal(firstTrace.parsedType, 'Program')
  assert.equal(firstTrace.projectConfig.moduleName, 'entry')
  assert.equal(firstTrace.projectConfig.compileMode, 'esmodule')
  assert.equal(firstTrace.projectConfig.arkTSVersion, '1.1')
  assert.equal(firstTrace.projectConfig.watchMode, 'false')
  assert.equal(firstTrace.projectConfig.perf, 1)
  assert.equal(firstTrace.hookEventFactoryEnabled, true)
  assert.equal(firstTrace.projectConfig.targetName, '.default')
  assert.equal(typeof firstTrace.projectConfig.compileSdkVersion, 'number')
  assert.equal(typeof firstTrace.projectConfig.compatibleSdkVersion, 'number')
  const canonicalProject = fs.realpathSync(project)
  assert.equal(
    firstTrace.projectConfig.modulePath,
    path.join(canonicalProject, 'entry').replaceAll(path.sep, '/'),
  )
  assert.equal(
    firstTrace.projectConfig.projectPath,
    path.join(canonicalProject, 'entry', 'src', 'main', 'ets').replaceAll(path.sep, '/'),
  )
  assert.deepEqual(firstTrace.projectConfig.deviceTypes, ['phone'])
  assert.deepEqual(firstTrace.projectConfigShape, {
    depName2DepInfoIsMap: true,
    dependencyAliasMapIsMap: true,
    permissionRequestCount: 0,
    permissionDefineCount: 0,
  })
  assert.deepEqual(firstTrace.depInfo, { enableIncre: false, changedDeps: [] })
  assert.equal(path.isAbsolute(firstTrace.projectConfig.cachePath), true)
  assert.equal(firstTrace.projectConfig.aceBuildJson.endsWith('/loader.json'), true)
  assert.equal(firstTrace.sourceFileCount > 0, true)
  assert.match(firstTrace.sourceHash, /^[0-9a-f]{16}$/)
  assert.equal(Number.isFinite(firstTrace.sourceMtimeMs), true)
  assert.equal(firstTrace.shareCacheBuildCount, 1)
  assert.equal(firstTrace.pluginCacheBuildCount, 1)
  assert.equal(firstTrace.importPackageName, 'entry')

  process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE = '1'
  await build(config)
  const secondTrace = JSON.parse(fs.readFileSync(tracePath, 'utf8'))
  assert.equal(secondTrace.shouldInvalidCacheCount > 0, true)
  assert.equal(typeof secondTrace.cacheRequest.id, 'string')
  assert.equal(secondTrace.cacheRequest.code.length > 0, true)
  assert.equal(secondTrace.cacheRequest.astType, 'Program')
  assert.equal(secondTrace.shareCacheBuildCount, 2)
  assert.equal(secondTrace.pluginCacheBuildCount, 2)

  const child = spawnSync(
    process.execPath,
    [
      '-e',
      `require(${JSON.stringify(path.resolve(__dirname, '..'))}).build(JSON.parse(process.env.ARKDOWN_CHILD_CONFIG)).catch((error) => { console.error(error); process.exitCode = 1 })`,
    ],
    {
      cwd: path.resolve(__dirname, '..'),
      env: { ...process.env, ARKDOWN_CHILD_CONFIG: JSON.stringify(config) },
      encoding: 'utf8',
    },
  )
  assert.equal(child.status, 0, `${child.stdout}\n${child.stderr}`)
  const childTrace = JSON.parse(fs.readFileSync(tracePath, 'utf8'))
  assert.equal(childTrace.shareCacheBuildCount, 3)
  assert.equal(childTrace.pluginCacheBuildCount, 3)
})

test('explicit JS plugins execute through the same native dispatcher', { skip: !enabled }, async (context) => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-explicit-plugin-'))
  const project = path.join(tempRoot, 'minimal-hap')
  context.after(() => fs.rmSync(tempRoot, { recursive: true, force: true }))
  fs.cpSync(path.join(workspaceRoot, 'tests', 'fixtures', 'minimal-hap'), project, { recursive: true })
  const events = []

  const result = await build(
    {
      project,
      target: 'hap',
      mode: 'debug',
      module: 'entry',
      product: 'default',
      buildTargetName: 'default',
      cache: false,
    },
    {
      name: 'explicit-lifecycle',
      buildStart() {
        events.push('buildStart')
      },
      transform(code) {
        return code
      },
      buildEnd(error) {
        assert.equal(error, undefined)
        events.push('buildEnd')
      },
    },
  )

  assert.equal(
    result.artifacts.some((artifact) => artifact.endsWith('.hap')),
    true,
  )
  assert.deepEqual(events, ['buildStart', 'buildEnd'])
})

test('compile-plugin cleanUp runs after a transform failure', { skip: !enabled }, async (context) => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-plugin-cleanup-'))
  const project = path.join(tempRoot, 'minimal-hap')
  const tracePath = path.join(tempRoot, 'plugin-trace.json')
  context.after(() => fs.rmSync(tempRoot, { recursive: true, force: true }))
  prepareCompilePluginProject(project)

  const priorTrace = process.env.ARKDOWN_PLUGIN_TRACE
  const priorThrow = process.env.ARKDOWN_PLUGIN_THROW_TRANSFORM
  process.env.ARKDOWN_PLUGIN_TRACE = tracePath
  process.env.ARKDOWN_PLUGIN_THROW_TRANSFORM = '1'
  context.after(() => {
    if (priorTrace === undefined) delete process.env.ARKDOWN_PLUGIN_TRACE
    else process.env.ARKDOWN_PLUGIN_TRACE = priorTrace
    if (priorThrow === undefined) delete process.env.ARKDOWN_PLUGIN_THROW_TRANSFORM
    else process.env.ARKDOWN_PLUGIN_THROW_TRANSFORM = priorThrow
  })

  await assert.rejects(
    build({
      project,
      target: 'hap',
      mode: 'debug',
      module: 'entry',
      product: 'default',
      buildTargetName: 'default',
      compilePluginPath: 'entry/compile-plugins/lifecycle.ts',
      cache: false,
    }),
    /intentional transform failure/,
  )
  const trace = JSON.parse(fs.readFileSync(tracePath, 'utf8'))
  assert.equal(trace.events.at(-1), 'cleanUp')
  assert.equal(trace.events.includes('afterBuildEnd'), false)
  assert.equal(trace.events.includes('bundleEnd'), false)
})
