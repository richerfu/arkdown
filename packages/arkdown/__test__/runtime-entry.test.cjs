'use strict'

const assert = require('node:assert/strict')
const test = require('node:test')

test('runtime entry forwards native config and a normalized plugin ABI', async (context) => {
  const indexPath = require.resolve('../index.js')
  const adapterPath = require.resolve('../js/plugin-adapter.js')
  const entryPath = require.resolve('../js/runtime-entry.js')
  const previousIndex = require.cache[indexPath]
  const previousAdapter = require.cache[adapterPath]
  const previousEntry = require.cache[entryPath]
  const calls = []
  const nativeBinding = {
    async build(config, plugins) {
      calls.push({ config, plugins })
      return { artifacts: ['entry.hap'], elapsedMs: 1 }
    },
    planRolldownPackagePlugins(plugins) {
      return { placements: plugins.map((_, pluginIndex) => ({ pluginIndex })) }
    },
  }
  require.cache[indexPath] = {
    id: indexPath,
    filename: indexPath,
    loaded: true,
    exports: nativeBinding,
  }
  delete require.cache[adapterPath]
  delete require.cache[entryPath]
  context.after(() => {
    if (previousIndex) require.cache[indexPath] = previousIndex
    else delete require.cache[indexPath]
    if (previousAdapter) require.cache[adapterPath] = previousAdapter
    else delete require.cache[adapterPath]
    if (previousEntry) require.cache[entryPath] = previousEntry
    else delete require.cache[entryPath]
  })

  const runtime = require('../js/runtime-entry.js')
  const plugin = { name: 'fixture', transform: (code) => code }
  const result = await runtime.build({ project: '/tmp/app', target: 'hap' }, plugin)

  assert.deepEqual(result.artifacts, ['entry.hap'])
  assert.equal(calls.length, 1)
  assert.deepEqual(calls[0].config, {
    project: '/tmp/app',
    target: 'hap',
    nodeExecutable: process.execPath,
  })
  assert.equal(calls[0].config.arktsVersion, undefined, 'the selected product owns the ArkTS default')
  assert.equal(calls[0].plugins.length, 1)
  assert.deepEqual(calls[0].plugins[0].__arkdownPluginHooks, ['transform'])
  assert.equal(typeof calls[0].plugins[0].__arkdownDispatchPluginHook, 'function')
  assert.equal(runtime.runHvigorCommand, undefined)
  assert.equal(runtime.hvigorVersion, undefined)
})
