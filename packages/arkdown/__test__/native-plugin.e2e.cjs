'use strict'

const assert = require('node:assert/strict')
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
  assert.deepEqual(firstTrace.events, ['buildStart', 'beforeBuildEnd', 'buildEnd'])
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

  process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE = '1'
  await build(config)
  const secondTrace = JSON.parse(fs.readFileSync(tracePath, 'utf8'))
  assert.equal(secondTrace.shouldInvalidCacheCount > 0, true)
  assert.equal(typeof secondTrace.cacheRequest.id, 'string')
  assert.equal(secondTrace.cacheRequest.code.length > 0, true)
  assert.equal(secondTrace.cacheRequest.astType, 'Program')
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
