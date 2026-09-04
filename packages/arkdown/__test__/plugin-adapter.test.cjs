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
  const hookEvents = []
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
  const share = {
    __arkdownBuildInstanceId: 'event-build',
    recordHookEvent(record) {
      hookEvents.push(record)
    },
    projectConfig: {
      moduleName: 'entry',
      perf: 1,
      depName2DepInfo: { library: { pkgRootPath: '/project/library' } },
      dependencyAliasMap: { alias: 'library' },
      mockParams: { source2ModuleIdMap: { './mock': '/project/mock.ts' } },
    },
    depInfo: { enableIncre: true, changedDeps: ['library'] },
  }
  await dispatch(normalized, 'seedContext', { share })
  share.setHashValueByFilePath('/tmp/Index.ets', 'source-hash')

  assert.deepEqual(normalized.__arkdownPluginHooks, PLUGIN_HOOKS)
  assert.equal(share.getHashByFilePath('/tmp/Index.ets'), 'source-hash')
  assert.equal(share.cache instanceof Map, true)
  assert.equal(share.commonCache instanceof Map, true)
  assert.equal(share.projectConfig.depName2DepInfo instanceof Map, true)
  assert.equal(share.projectConfig.dependencyAliasMap instanceof Map, true)
  assert.equal(share.projectConfig.mockParams.source2ModuleIdMap instanceof Map, true)
  assert.deepEqual(share.depInfo, { enableIncre: true, changedDeps: ['library'] })
  assert.equal(typeof share.getLogger('fixture').printErrorAndExit, 'function')
  const event = share.getHookEventFactory('fixture', 'buildStart').createEvent('compile')
  const subEvent = event.createSubEvent('parse')
  assert.equal(event.start(10), event)
  assert.equal(subEvent.startAsyncEvent(11), subEvent)
  subEvent.stopAsyncEvent('success', 12)
  event.stop('warn', 20)
  event.setTotalTime(7)
  assert.deepEqual(hookEvents, [
    {
      eventId: 'event-build:1',
      pluginName: 'fixture',
      hookName: 'buildStart',
      eventName: 'compile',
      operation: 'start',
      timestampMs: 10,
    },
    {
      eventId: 'event-build:2',
      parentEventId: 'event-build:1',
      pluginName: 'fixture',
      hookName: 'buildStart',
      eventName: 'parse',
      operation: 'startAsyncEvent',
      timestampMs: 11,
    },
    {
      eventId: 'event-build:2',
      parentEventId: 'event-build:1',
      pluginName: 'fixture',
      hookName: 'buildStart',
      eventName: 'parse',
      operation: 'stopAsyncEvent',
      timestampMs: 12,
      state: 'success',
    },
    {
      eventId: 'event-build:1',
      pluginName: 'fixture',
      hookName: 'buildStart',
      eventName: 'compile',
      operation: 'stop',
      timestampMs: 20,
      state: 'warn',
    },
    {
      eventId: 'event-build:1',
      pluginName: 'fixture',
      hookName: 'buildStart',
      eventName: 'compile',
      operation: 'setTotalTime',
      totalTimeMs: 7,
    },
  ])
  assert.deepEqual(await dispatch(normalized, 'beforeBuild'), {})
  assert.deepEqual((await dispatch(normalized, 'buildStart', { buildStartOptions: { input: 'index.ets' } })), {})
  assert.deepEqual(await dispatch(normalized, 'afterBuildStart'), {})
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
  assert.deepEqual(dispatch(normalized, 'afterBuildEnd'), {})
  assert.deepEqual(await dispatch(normalized, 'bundleEnd'), {})
  assert.deepEqual(dispatch(normalized, 'cleanUp'), {})
  assert.deepEqual(
    dispatch(normalized, 'shouldInvalidCache', {
      cacheRequest: { id: 'index.ets', code: 'let a = 1', __arkdownOxcAst: { node: { type: 'Program' } } },
    }),
    { shouldInvalidCache: true },
  )
  assert.equal(calls.length, PLUGIN_HOOKS.length)
  assert.equal(calls.every((call) => call.share === share), true)
})

test('hook event factories follow ets-loader AnalyzeMode gating', () => {
  for (const [perf, enabled] of [
    [0, false],
    [1, true],
    [2, false],
    [3, true],
  ]) {
    let observedShare
    const plugin = normalizePlugin(
      {
        name: `perf-${perf}`,
        buildStart() {
          observedShare = this.share
        },
      },
      fakeBinding(),
    )
    dispatch(plugin, 'seedContext', {
      share: { __arkdownBuildInstanceId: `perf-build-${perf}`, projectConfig: { perf } },
    })
    dispatch(plugin, 'buildStart')
    assert.equal(observedShare.getHookEventFactory('fixture', 'buildStart') !== undefined, enabled)
  }
})

test('plugins in one native build observe the same mutable hvigor share object', () => {
  let firstShare
  let secondShare
  let observedMarker
  const first = normalizePlugin(
    {
      name: 'share-writer',
      beforeBuild() {
        firstShare = this.share
        this.share.ModuleSourceFile = [{ id: 'Index.ets' }]
        this.share.customMarker = 'visible-to-next-plugin'
      },
    },
    fakeBinding(),
    { compilePlugin: true, pluginFile: '/plugins/shared.ts' },
  )
  const second = normalizePlugin(
    {
      name: 'share-reader',
      beforeBuild() {
        secondShare = this.share
        observedMarker = this.share.customMarker
      },
    },
    fakeBinding(),
    { compilePlugin: true, pluginFile: '/plugins/shared.ts' },
  )

  dispatch(first, 'seedContext', {
    share: { __arkdownBuildInstanceId: 'shared-build', projectConfig: { projectTopDir: '/project' } },
  })
  dispatch(second, 'seedContext', {
    share: { __arkdownBuildInstanceId: 'shared-build', projectConfig: { projectTopDir: '/project' } },
  })
  dispatch(first, 'beforeBuild')
  dispatch(second, 'beforeBuild')

  assert.equal(firstShare, secondShare)
  assert.equal(observedMarker, 'visible-to-next-plugin')
  assert.deepEqual(secondShare.ModuleSourceFile, [{ id: 'Index.ets' }])
})

test('plugin and share caches survive a V8 snapshot restored by a later build process scope', () => {
  const createPlugin = (observed) =>
    normalizePlugin(
      {
        name: 'persistent-cache',
        buildStart() {
          const count = (this.cache.get('count') || 0) + 1
          this.cache.set('count', count)
          this.share.cache.set('shared-count', (this.share.cache.get('shared-count') || 0) + 1)
          observed.push([count, this.share.cache.get('shared-count')])
        },
      },
      fakeBinding(),
      { compilePlugin: true, pluginFile: '/plugins/persistent.ts' },
    )

  const firstObserved = []
  const first = createPlugin(firstObserved)
  dispatch(first, 'seedContext', {
    share: { __arkdownBuildInstanceId: 'cache-build-1', projectConfig: { projectTopDir: '/scope-one' } },
  })
  dispatch(first, 'buildStart')
  const snapshot = dispatch(first, 'snapshotContext').cacheSnapshot

  const secondObserved = []
  const second = createPlugin(secondObserved)
  dispatch(second, 'seedContext', {
    share: {
      __arkdownBuildInstanceId: 'cache-build-2',
      __arkdownPluginCacheSnapshot: snapshot,
      projectConfig: { projectTopDir: '/scope-two' },
    },
  })
  dispatch(second, 'buildStart')

  assert.deepEqual(firstObserved, [[1, 1]])
  assert.deepEqual(secondObserved, [[2, 2]])
})

test('ordered hook objects retain handler receiver and native order metadata', () => {
  const calls = []
  const plugin = normalizePlugin(
    {
      name: 'ordered-hooks',
      transform: {
        order: 'post',
        handler(code, id) {
          calls.push({ name: this.share.projectConfig.moduleName, id })
          return `${code}\n// ordered`
        },
      },
      beforeBuildEnd: {
        order: 'pre',
        handler() {
          calls.push({ name: 'beforeBuildEnd' })
        },
      },
    },
    fakeBinding(),
  )
  dispatch(plugin, 'seedContext', { share: { projectConfig: { moduleName: 'entry' } } })

  assert.deepEqual(plugin.__arkdownPluginHookMeta, [
    { name: 'transform', order: 'post' },
    { name: 'beforeBuildEnd', order: 'pre' },
  ])
  assert.deepEqual(dispatch(plugin, 'transform', { code: 'export {}', id: 'Index.ets' }), {
    transform: 'export {}\n// ordered',
  })
  assert.deepEqual(dispatch(plugin, 'beforeBuildEnd'), {})
  assert.deepEqual(calls, [
    { name: 'entry', id: 'Index.ets' },
    { name: 'beforeBuildEnd' },
  ])
})

test('plugin context delegates graph operations to Rolldown and retains hvigor cache/async state', async () => {
  const calls = []
  let observed
  const nativeContext = {
    resolve: async (source, importer, options) => {
      calls.push(['resolve', source, importer, options])
      return { id: '/project/dep.ts', external: false }
    },
    load: async (options) => {
      calls.push(['load', options])
      return { id: options.id }
    },
    emitFile: (file) => {
      calls.push(['emitFile', file])
      return 'asset-reference'
    },
    addWatchFile: (id) => calls.push(['addWatchFile', id]),
    getModuleIds: () => ['/project/index.ts'],
    getModuleInfo: (id) => ({ id }),
    getFileName: (id) => `${id}.json`,
    getCombinedSourcemap: () => ({ version: 3, sources: ['/project/index.ts'], mappings: 'AAAA' }),
    parse: () => ({ ast: { node: { type: 'Program', body: [] }, fixes: [] }, comments: [] }),
    info: (message) => calls.push(['info', message]),
    warn: (message) => calls.push(['warn', message]),
    debug: (message) => calls.push(['debug', message]),
  }
  const plugin = normalizePlugin(
    {
      name: 'context-plugin',
      async buildStart() {
        this.cache.set('answer', 42)
        const resolved = await this.resolve('./dep', '/project/index.ts', {
          kind: 'import-statement',
          custom: { owner: 'context-plugin' },
        })
        const loaded = await this.load({ id: resolved.id })
        const reference = this.emitFile({ type: 'asset', name: 'meta', source: '{}' })
        this.addWatchFile('/project/config.json')
        this.info({ message: 'context ready' })
        observed = {
          cache: this.cache.get('answer'),
          resolved: resolved.id,
          loaded: loaded.id,
          reference,
          fileName: this.getFileName(reference),
          watchMode: this.meta.watchMode,
        }
      },
      transform(code, id, options) {
        const pluginContext = this
        this.async(() => setImmediate(() => {
          calls.push(['async-complete'])
          pluginContext.signal()
        }))
        calls.push(['transformContext', id, options, this.getCombinedSourcemap()])
        return code
      },
    },
    fakeBinding(),
    { compilePlugin: true },
  )
  dispatch(plugin, 'seedContext', { share: { projectConfig: { watchMode: true } } })

  await dispatch(plugin, 'buildStart', { context: nativeContext })
  dispatch(plugin, 'transform', {
    context: nativeContext,
    code: 'export {}',
    id: '/project/index.ts',
    transformOptions: { moduleType: 'ts' },
  })
  await dispatch(plugin, 'bundleEnd', { context: nativeContext })

  assert.deepEqual(observed, {
    cache: 42,
    resolved: '/project/dep.ts',
    loaded: '/project/dep.ts',
    reference: 'asset-reference',
    fileName: 'asset-reference.json',
    watchMode: true,
  })
  assert.deepEqual(calls, [
    ['resolve', './dep', '/project/index.ts', { kind: 'import-statement', custom: { owner: 'context-plugin' } }],
    ['load', { id: '/project/dep.ts' }],
    ['emitFile', { type: 'asset', name: 'meta', source: '{}' }],
    ['addWatchFile', '/project/config.json'],
    ['info', 'context ready'],
    [
      'transformContext',
      '/project/index.ts',
      { moduleType: 'ts' },
      { version: 3, sources: ['/project/index.ts'], mappings: 'AAAA' },
    ],
    ['async-complete'],
  ])
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
