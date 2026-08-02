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
      assert.equal(typeof runtimeTasks.tasksJson, 'string')
      assert.equal(typeof runtimeTasks.selectionJson, 'string')
      assert.equal(typeof runtimeTasks.invoke, 'function')
      return { artifacts: ['entry.hap'], elapsedMs: 1 }
    },
    collectBuildScriptPluginPaths: actualNativeBinding.collectBuildScriptPluginPaths,
    inspectHvigorContext() {
      return '{}'
    },
    discoverBuildScriptFiles: actualNativeBinding.discoverBuildScriptFiles,
    compileHvigorScript: actualNativeBinding.compileHvigorScript,
    compileHvigorScriptSync: actualNativeBinding.compileHvigorScriptSync,
    hvigorSyncVersionChanged: actualNativeBinding.hvigorSyncVersionChanged,
    hvigorProjectCacheHash: actualNativeBinding.hvigorProjectCacheHash,
    planHvigorCompilePlugins: actualNativeBinding.planHvigorCompilePlugins,
    planRolldownPackagePlugins: actualNativeBinding.planRolldownPackagePlugins,
    loadBuildScriptModuleModel: actualNativeBinding.loadBuildScriptModuleModel,
    loadBuildScriptRuntimeModel: actualNativeBinding.loadBuildScriptRuntimeModel,
    parseHvigorJsonText: actualNativeBinding.parseHvigorJsonText,
    mergeHvigorConfig: actualNativeBinding.mergeHvigorConfig,
    planBuildScriptRuntime: actualNativeBinding.planBuildScriptRuntime,
    prepareNativeBuildConfig: actualNativeBinding.prepareNativeBuildConfig,
    runHvigorClean: actualNativeBinding.runHvigorClean,
    runHvigorPrune: actualNativeBinding.runHvigorPrune,
    runtimeCommandPlan: actualNativeBinding.runtimeCommandPlan,
    runtimeTaskCatalog: actualNativeBinding.runtimeTaskCatalog,
    runtimeTaskReport: actualNativeBinding.runtimeTaskReport,
    resolveBuildScriptPluginPath: actualNativeBinding.resolveBuildScriptPluginPath,
    validateBuildScriptBuildOption: actualNativeBinding.validateBuildScriptBuildOption,
    validateHvigorCompilePlugins: actualNativeBinding.validateHvigorCompilePlugins,
    writeHvigorBuildMeta: actualNativeBinding.writeHvigorBuildMeta,
    writeHvigorSyncModel: actualNativeBinding.writeHvigorSyncModel,
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
