'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')

const {
  PLUGIN_HOOKS,
  PluginAdapter,
  PluginModuleLoader,
  normalizePlugin,
  normalizePluginList,
} = require('../js/plugin-adapter.js')

function fakeBinding() {
  return {
    compilePluginModule: async (file) => `module.exports = { default: { name: ${JSON.stringify(path.basename(file))}, transform(code) { return code } } }`,
    compilePluginModuleSync: (file) =>
      `module.exports = { value: ${JSON.stringify(path.basename(file))}, default: { name: 'nested' } }`,
    parsePluginAst: () => ({ ast: { node: { type: 'Program', body: [] }, fixes: [] }, comments: [] }),
    planRolldownPackagePlugins(plugins) {
      return { placements: plugins.map((_, pluginIndex) => ({ pluginIndex })) }
    },
    validateHvigorCompilePlugins() {
      return {}
    },
    planHvigorCompilePlugins(_config, plugins) {
      return { placements: plugins.map((_, pluginIndex) => ({ pluginIndex })) }
    },
  }
}

function dispatch(plugin, hookName, request = {}) {
  return plugin.__arkdownDispatchPluginHook({ hookName, ...request })
}

test('adapter preserves the complete ArkTS 1.1 compile-plugin hook surface', async () => {
  const calls = []
  const plugin = Object.fromEntries(
    PLUGIN_HOOKS.map((hookName) => [
      hookName,
      function (...args) {
        calls.push({ hookName, args, share: this.share })
        if (hookName === 'resolveId' || hookName === 'resolveDynamicImport') return './resolved.js'
        if (hookName === 'load') return 'export const loaded = true'
        if (hookName === 'transform') return { code: `${args[0]}\n// transformed` }
        if (hookName === 'shouldInvalidCache') return true
        return undefined
      },
    ]),
  )
  plugin.name = 'all-hooks'
  const normalized = normalizePlugin(plugin, fakeBinding())
  const share = { projectConfig: { moduleName: 'entry' } }
  await dispatch(normalized, 'seedContext', { share })

  assert.deepEqual(normalized.__arkdownPluginHooks, PLUGIN_HOOKS)
  assert.deepEqual((await dispatch(normalized, 'buildStart', { buildStartOptions: { input: 'index.ets' } })), {})
  assert.deepEqual(dispatch(normalized, 'resolveId', { id: 'dep', importer: 'index.ets', resolveOptions: {} }), {
    resolveId: './resolved.js',
  })
  assert.deepEqual(dispatch(normalized, 'resolveDynamicImport', { id: 'lazy', importer: 'index.ets' }), {
    resolveId: './resolved.js',
  })
  assert.deepEqual(dispatch(normalized, 'load', { id: './resolved.js' }), { load: 'export const loaded = true' })
  assert.deepEqual(dispatch(normalized, 'transform', { code: 'let a = 1', id: 'index.ets' }), {
    transform: { code: 'let a = 1\n// transformed' },
  })
  assert.deepEqual(dispatch(normalized, 'moduleParsed', { moduleInfo: { id: 'index.ets' } }), {})
  assert.deepEqual(dispatch(normalized, 'beforeBuildEnd'), {})
  assert.deepEqual(dispatch(normalized, 'buildEnd', { buildEndContext: { errors: [] } }), {})
  assert.deepEqual(
    dispatch(normalized, 'shouldInvalidCache', {
      cacheRequest: { id: 'index.ets', code: 'let a = 1', __arkdownOxcAst: { node: { type: 'Program' } } },
    }),
    { shouldInvalidCache: true },
  )
  assert.equal(calls.length, PLUGIN_HOOKS.length)
  assert.equal(calls.every((call) => call.share === share), true)
})

test('explicit plugin factories and nested arrays retain deterministic native order', () => {
  const binding = fakeBinding()
  binding.planRolldownPackagePlugins = (plugins) => ({
    placements: [1, 0].map((pluginIndex) => ({ pluginIndex })),
  })
  const plugins = normalizePluginList(
    () => [
      { name: 'last', transform: (code) => code },
      [{ name: 'first', load: () => null }],
    ],
    binding,
  )

  assert.deepEqual(
    plugins.map((plugin) => plugin.name),
    ['first', 'last'],
  )
})

test('configured CJS plugin modules use default exports and path fields do not reach Rust build config', async (context) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-plugin-adapter-'))
  const pluginFile = path.join(root, 'compile-plugin.cjs')
  fs.writeFileSync(pluginFile, "exports.default = { name: 'configured', transform(code) { return code } }\n")
  context.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const prepared = await new PluginAdapter(fakeBinding()).prepare({
    project: root,
    target: 'hap',
    compilePluginPath: './compile-plugin.cjs',
  })

  assert.deepEqual(prepared.config, { project: root, target: 'hap' })
  assert.equal(prepared.plugins.length, 1)
  assert.equal(prepared.plugins[0].name, 'configured')
  assert.equal(prepared.plugins[0].__arkdownHvigorCompilePlugin, true)
})

test('OXC plugin loader accepts TS, MTS, and CTS entry modules', async (context) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-plugin-module-'))
  context.after(() => fs.rmSync(root, { recursive: true, force: true }))
  const loader = new PluginModuleLoader(fakeBinding())

  for (const extension of ['.ts', '.mts', '.cts']) {
    const file = path.join(root, `plugin${extension}`)
    fs.writeFileSync(file, 'export default {}\n')
    const loaded = await loader.load(file, root)
    assert.equal(loaded.exports.default.name, `plugin${extension}`)
  }
})

test('output-stage hooks are rejected instead of being silently dropped', () => {
  assert.throws(
    () => normalizePluginList({ name: 'chunk-plugin', renderChunk() {} }, fakeBinding()),
    (error) => error.code === 'ARKDOWN_PLUGIN_HOOK_UNSUPPORTED' && error.hookName === 'renderChunk',
  )
})

test('non-function hooks are rejected with a typed diagnostic', () => {
  assert.throws(
    () => normalizePluginList({ name: 'invalid-plugin', transform: true }, fakeBinding()),
    (error) => error.code === 'ARKDOWN_PLUGIN_HOOK_INVALID' && error.hookName === 'transform',
  )
})

test('removed Hvigor task runtime options fail before native build dispatch', async () => {
  await assert.rejects(
    () => new PluginAdapter(fakeBinding()).prepare({ project: '/tmp/app', tasks: ['assembleHap'] }),
    (error) => error.code === 'ARKDOWN_HVIGOR_RUNTIME_UNSUPPORTED' && error.option === 'tasks',
  )
})
