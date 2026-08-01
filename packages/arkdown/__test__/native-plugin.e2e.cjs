'use strict'

const assert = require('node:assert/strict')
const childProcess = require('node:child_process')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')

const { build } = require('..')

const workspaceRoot = path.resolve(__dirname, '..', '..', '..')
const enabled = process.env.ARKDOWN_NATIVE_PLUGIN_E2E === '1'
const verifyHvigor = process.env.ARKDOWN_HVIGOR_PLUGIN_PARITY === '1'

function prepareCompilePluginProject(project) {
  fs.cpSync(path.join(workspaceRoot, 'tests', 'fixtures', 'minimal-hap'), project, { recursive: true })
  const compileRoot = path.join(project, 'entry', 'compile-plugins')
  fs.mkdirSync(compileRoot, { recursive: true })
  fs.copyFileSync(
    path.join(workspaceRoot, 'tests', 'fixtures', 'compile-plugins', 'lifecycle.ts'),
    path.join(compileRoot, 'lifecycle.ts'),
  )
  fs.writeFileSync(
    path.join(project, 'entry', 'hvigorfile.ts'),
    `
import { getNode } from '@ohos/hvigor'
import { hapTasks, OhosPluginId } from '@ohos/hvigor-ohos-plugin'

const loadCompilePlugin = {
  pluginId: 'fixture.load-compile-plugin',
  apply(node: any) {
    node.getContext(OhosPluginId.OHOS_HAP_PLUGIN)
      .loadCompilePlugin('./compile-plugins/lifecycle.ts')
  },
}

export default { system: hapTasks, plugins: [loadCompilePlugin] }
`,
  )
}

test('OhosModuleContext TypeScript compile plugin executes through the Rolldown driver', { skip: !enabled }, async (context) => {
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
    cache: true,
  }
  let hvigorTrace
  if (verifyHvigor) {
    const hvigorProject = path.join(tempRoot, 'hvigor-minimal-hap')
    const hvigorTracePath = path.join(tempRoot, 'hvigor-plugin-trace.json')
    prepareCompilePluginProject(hvigorProject)
    const hvigorw = process.env.ARKDOWN_PARITY_HVIGORW
    assert.ok(hvigorw, 'ARKDOWN_PARITY_HVIGORW is required when ARKDOWN_HVIGOR_PLUGIN_PARITY=1')
    const command = childProcess.spawnSync(
      hvigorw,
      ['--mode', 'module', '-p', 'product=default', '-p', 'buildMode=debug', 'assembleHap', '--no-daemon'],
      {
        cwd: hvigorProject,
        encoding: 'utf8',
        env: { ...process.env, ARKDOWN_PLUGIN_TRACE: hvigorTracePath },
      },
    )
    assert.equal(command.status, 0, `latest hvigor failed:\n${command.stdout}\n${command.stderr}`)
    hvigorTrace = JSON.parse(fs.readFileSync(hvigorTracePath, 'utf8'))
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
  assert.equal(firstTrace.moduleIds.length > 0, true)
  assert.equal(firstTrace.moduleInfoCount, firstTrace.moduleIds.length)
  assert.equal(firstTrace.parsedType, 'Program')
  assert.equal(firstTrace.projectConfig.moduleName, 'entry')
  if (hvigorTrace) {
    assert.deepEqual(firstTrace.events, hvigorTrace.events)
    assert.equal(firstTrace.buildStartArgumentCount, hvigorTrace.buildStartArgumentCount)
    assert.equal(firstTrace.beforeBuildEndArgumentCount, hvigorTrace.beforeBuildEndArgumentCount)
    assert.equal(firstTrace.buildEndArgumentCount, hvigorTrace.buildEndArgumentCount)
    assert.equal(firstTrace.parsedType, hvigorTrace.parsedType)
    assert.equal(hvigorTrace.resolveIdCount > 0, true)
    assert.equal(hvigorTrace.loadCount > 0, true)
    assert.equal(hvigorTrace.transformCount > 0, true)
    assert.equal(hvigorTrace.moduleParsedCount > 0, true)
  }

  process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE = '1'
  await build(config)
  const secondTrace = JSON.parse(fs.readFileSync(tracePath, 'utf8'))
  assert.equal(secondTrace.shouldInvalidCacheCount > 0, true)
  assert.equal(typeof secondTrace.cacheRequest.id, 'string')
  assert.equal(secondTrace.cacheRequest.id.length > 0, true)
  assert.equal(secondTrace.cacheRequest.code.length > 0, true)
  assert.equal(secondTrace.cacheRequest.astType, 'Program')
})
