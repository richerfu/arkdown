'use strict'

const assert = require('node:assert/strict')
const path = require('node:path')
const test = require('node:test')

test('runtime entry keeps runHvigorCommand bound to the raw native build', async (context) => {
  const indexPath = require.resolve('../index.js')
  const entryPath = require.resolve('../runtime-entry.js')
  const actualNativeBinding = require('../index.js')
  const previousIndex = require.cache[indexPath]
  const previousEntry = require.cache[entryPath]
  const calls = []
  const nativeBinding = {
    async build(config, _plugins, runtimeTasks) {
      calls.push(config)
      assert.ok(Array.isArray(runtimeTasks.tasks))
      assert.equal(typeof runtimeTasks.selection, 'object')
      assert.equal(typeof runtimeTasks.dispatchTask, 'function')
      assert.equal(typeof runtimeTasks.transformAbc, 'function')
      assert.equal(typeof runtimeTasks.modifyNativeCommand, 'function')
      return { artifacts: ['entry.hap'], elapsedMs: 1 }
    },
    RuntimeBuildScriptSession: actualNativeBinding.RuntimeBuildScriptSession,
    installRuntimeObservability() {},
    finishRuntimeObservability() {},
    persistRuntimeReport() {},
    persistRuntimeDetails() {},
    runtimeObservabilityIsEnabled() {
      return false
    },
    emitRuntimeLog() {},
    compileHvigorScript: actualNativeBinding.compileHvigorScript,
    compileHvigorScriptSync: actualNativeBinding.compileHvigorScriptSync,
    hvigorProjectCacheHash: actualNativeBinding.hvigorProjectCacheHash,
    planHvigorCompilePlugins: actualNativeBinding.planHvigorCompilePlugins,
    planRolldownPackagePlugins: actualNativeBinding.planRolldownPackagePlugins,
    parseHvigorJsonText: actualNativeBinding.parseHvigorJsonText,
    mergeHvigorConfig: actualNativeBinding.mergeHvigorConfig,
    RuntimeFileSnapshot: actualNativeBinding.RuntimeFileSnapshot,
    RuntimeTaskCacheSession: actualNativeBinding.RuntimeTaskCacheSession,
    RuntimeTaskSnapshot: actualNativeBinding.RuntimeTaskSnapshot,
    validateHvigorCompilePlugins: actualNativeBinding.validateHvigorCompilePlugins,
  }
  require.cache[indexPath] = {
    id: indexPath,
    filename: indexPath,
    loaded: true,
    exports: nativeBinding,
  }
  delete require.cache[entryPath]
  context.after(() => {
    if (previousIndex) require.cache[indexPath] = previousIndex
    else delete require.cache[indexPath]
    if (previousEntry) require.cache[entryPath] = previousEntry
    else delete require.cache[entryPath]
  })

  const runtime = require('../runtime-entry.js')
  const project = path.resolve(__dirname, '../../../tests/fixtures/minimal-hap')
  const result = await runtime.runHvigorCommand({
    project,
    tasks: ['assembleHap'],
    executionMode: 'module',
    module: 'entry@default',
  })

  assert.deepEqual(result.artifacts, ['entry.hap'])
  assert.equal(calls.length, 1)
  assert.equal(calls[0].target, 'hap')
  assert.equal(calls[0].module, 'entry')
})
