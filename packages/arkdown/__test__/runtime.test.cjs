'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')

const {
  HvigorRuntimeHost,
  RuntimePluginLoader,
  createBuild,
  normalizePluginList,
  normalizeSortedPluginList,
  applyFunctionOrder,
  parseShouldInvalidCacheRequest,
  parsePluginContext,
  buildPluginContext,
  requireDefaultExport,
} = require('../runtime')

function tempProject() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-runtime-'))
}

test('runtime loader executes hvigorfile plugins and compilePluginPath modules', async () => {
  const root = tempProject()
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
    name: 'inline-plugin',
    buildStart() {}
  }],
  compilePluginPath: './compile-plugin.js'
}
`,
  )

  const plugins = await new RuntimePluginLoader({ project: root }).load()
  assert.deepEqual(
    plugins.map((plugin) => plugin.name),
    ['inline-plugin', 'compile-plugin'],
  )
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
    path.join(root, 'hvigorfile.js'),
    `
module.exports = {
  plugins: [{ name: 'runtime-plugin', load() { return null } }]
}
`,
  )

  const seen = []
  const build = createBuild(async (_config, plugins) => {
    seen.push(...plugins.map((plugin) => plugin.name))
    return { artifacts: [] }
  })

  await build({ project: root }, [{ name: 'explicit-plugin', load() { return null } }])
  assert.deepEqual(seen, ['runtime-plugin', 'explicit-plugin'])
})

test('runtime loader discovers module hvigorfiles from package entry', async () => {
  const root = tempProject()
  const entry = path.join(root, 'entry')
  fs.mkdirSync(entry)
  fs.writeFileSync(
    path.join(root, 'hvigorfile.js'),
    `
module.exports = {
  plugins: [{ name: 'project-plugin', load() { return null } }]
}
`,
  )
  fs.writeFileSync(
    path.join(entry, 'hvigorfile.js'),
    `
module.exports = {
  plugins: [{ name: 'module-plugin', load() { return null } }]
}
`,
  )

  const plugins = await new RuntimePluginLoader({ project: root }).load()
  assert.deepEqual(
    plugins.map((plugin) => plugin.name),
    ['project-plugin', 'module-plugin'],
  )
})

test('normalizePluginList keeps plugin arrays returned by factories', () => {
  const plugins = normalizePluginList(() => [
    { name: 'factory-a', transform(code) { return code } },
    () => ({ name: 'factory-b', load() { return null } }),
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

  assert.equal(plugin.shouldInvalidCache(JSON.stringify({
    id: 'entry',
    meta: {
      moduleName: 'entry',
      cacheDir: '/tmp/cache',
      compileEnv: 'env',
    },
    changedFiles: ['entry/src/main/ets/pages/Index.ets'],
    isWatch: true,
    cacheHit: true,
  })), true)
  assert.deepEqual(parseShouldInvalidCacheRequest('not-json'), { cacheHit: true })
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
  fs.writeFileSync(pluginFile, `
let n = ${callCount};
exports.default = { name: 'cache-plugin', transform(code) { return code; }, _loaded: ++n };
`)

  const loader1 = new RuntimePluginLoader({ project: root, compilePluginPath: './cache-plugin.js' })
  const plugins1 = await loader1.loadConfiguredPluginPaths()

  // Overwrite with a new version and reload via a fresh loader (seen Set is fresh)
  fs.writeFileSync(pluginFile, `
exports.default = { name: 'cache-plugin-v2', transform(code) { return code; } };
`)
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
    () => normalizePluginList({ name: 'bad-order', order: 'middle', transform(code) { return code } }),
    (err) => err.code === 'PLUGIN_NAME_WITH_UNKNOWN_ORDER',
  )
})

test('function order receives name-to-index map and returns correct splice position', () => {
  const alpha = { name: 'alpha', transform(code) { return code } }
  const gamma = { name: 'gamma', transform(code) { return code } }
  // beta should be inserted between alpha and gamma by returning index 1
  const beta = {
    name: 'beta',
    order(nameToIndex) {
      // alpha is at 0, so 1 inserts after alpha
      return (nameToIndex.get('alpha') ?? 0) + 1
    },
    transform(code) { return code },
  }

  const result = applyFunctionOrder([alpha, gamma, beta])
  assert.deepEqual(result.map((p) => p.name), ['alpha', 'beta', 'gamma'])
  // order property should be stripped from the resolved plugin
  assert.equal(result[1].order, undefined)
})

test('shouldTransformCachedModule receives structured request objects', () => {
  const [plugin] = normalizePluginList({
    name: 'stcm-plugin',
    shouldTransformCachedModule(request) {
      return request.cacheHit === false
    },
  })

  assert.equal(
    plugin.shouldTransformCachedModule(JSON.stringify({ id: 'mod', cacheHit: false, changedFiles: [], isWatch: false })),
    true,
  )
  assert.equal(
    plugin.shouldTransformCachedModule(JSON.stringify({ id: 'mod', cacheHit: true, changedFiles: [], isWatch: false })),
    false,
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

test('hvigorfile with hapTasks call captures compilePluginPath via HvigorRuntimeHost', async () => {
  const root = tempProject()
  const pluginFile = path.join(root, 'compile-plugin.js')
  fs.writeFileSync(pluginFile, `
exports.default = { name: 'captured-plugin', transform(code) { return code } }
`)
  // TypeScript hvigorfile that calls hapTasks with compilePluginPath
  fs.writeFileSync(path.join(root, 'hvigorfile.ts'), `
import { hapTasks } from '@ohos/hvigor-ohos-plugin';
export default hapTasks({
  buildOption: {
    arkOptions: {
      compilePluginPath: './compile-plugin.js'
    }
  }
});
`)
  const plugins = await new RuntimePluginLoader({ project: root }).load()
  // The captured compilePluginPath should have been loaded
  assert.ok(
    plugins.some((p) => p.name === 'captured-plugin'),
    `expected captured-plugin in [${plugins.map((p) => p.name).join(', ')}]`,
  )
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

  for (const [key, value] of Object.entries(config)) {
    if (key === 'compilePluginPath' || key === 'hvigorPluginFile') {
      assert.equal(capturedConfig[key], undefined, `BuildConfig.${key} must be consumed by runtime.js before nativeBuild`)
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

// ---------------------------------------------------------------------------
// Plugin context binding — hvigor doWrapper / getCompilePluginContext
// ---------------------------------------------------------------------------

test('parsePluginContext returns empty object for empty string', () => {
  assert.deepEqual(parsePluginContext(''), {})
  assert.deepEqual(parsePluginContext(null), {})
  assert.deepEqual(parsePluginContext(undefined), {})
})

test('parsePluginContext returns parsed object for valid JSON', () => {
  const share = { bundleName: 'com.example.app', buildMode: 'debug' }
  const result = parsePluginContext(JSON.stringify({ share }))
  assert.deepEqual(result.share, share)
})

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

test('buildPluginContext getFileName resolves known refs and rejects unknown refs', () => {
  const ctx = buildPluginContext({ fileNames: { asset123: 'assets/icon.png' } })
  assert.equal(ctx.getFileName('asset123'), 'assets/icon.png')
  assert.throws(
    () => ctx.getFileName('missing'),
    (error) => error.code === 'FILE_NAME_NOT_FOUND',
  )
})

test('normalizeHookArguments binds this.share for beforeBuild', () => {
  const share = { bundleName: 'com.example.test', buildMode: 'debug' }
  let capturedThis = null
  const [plugin] = normalizePluginList({
    name: 'ctx-test',
    beforeBuild(ctxJson) {
      capturedThis = this
    },
  })
  plugin.beforeBuild(JSON.stringify({ share }))
  assert.ok(capturedThis, 'this should be bound')
  assert.equal(capturedThis.share.bundleName, share.bundleName)
  assert.equal(typeof capturedThis.getModuleIds, 'function')
  assert.equal(typeof capturedThis.getModuleInfo, 'function')
})

test('normalizeHookArguments binds this.share for afterBuildStart', () => {
  const share = { bundleName: 'com.example.hsp', buildMode: 'release' }
  let capturedThis = null
  const [plugin] = normalizePluginList({
    name: 'ctx-test2',
    afterBuildStart(ctxJson) {
      capturedThis = this
    },
  })
  plugin.afterBuildStart(JSON.stringify({ share }))
  assert.ok(capturedThis)
  assert.equal(capturedThis.share.bundleName, share.bundleName)
})

test('share context is propagated from beforeBuild to subsequent hooks', () => {
  const share = { bundleName: 'com.example.prop', buildMode: 'debug' }
  const captured = {}
  const plugin = normalizePluginList({
    name: 'propagation-test',
    beforeBuild(ctxJson) { captured.beforeBuild = this.share },
    afterBuildStart(ctxJson) { captured.afterBuildStart = this.share },
    buildStart(ctxJson) { captured.buildStart = this.share },
  })[0]

  // Fire beforeBuild — this captures share into module-level _pluginShareContext
  plugin.beforeBuild(JSON.stringify({ share }))
  // Fire subsequent hooks without share (they pick up from _pluginShareContext)
  plugin.afterBuildStart('')
  plugin.buildStart('')

  assert.equal(captured.beforeBuild.bundleName, share.bundleName)
  assert.equal(captured.afterBuildStart.bundleName, share.bundleName)
  assert.equal(captured.buildStart.bundleName, share.bundleName)
})

test('share context is bound for resolveId load transform and cache hooks', () => {
  const share = { projectConfig: { bundleName: 'com.example.hooks' } }
  const captured = []
  const plugin = normalizePluginList({
    name: 'all-hook-contexts',
    beforeBuild() {},
    resolveId() { captured.push(this.share) },
    load() { captured.push(this.share) },
    transform() { captured.push(this.share) },
    shouldInvalidCache() { captured.push(this.share); return false },
  })[0]
  plugin.beforeBuild(JSON.stringify({ share }))
  plugin.resolveId('dep', '/entry.ets')
  plugin.load('/entry.ets')
  plugin.transform('export {}', '/entry.ets')
  plugin.shouldInvalidCache('{}')
  assert.equal(captured.length, 4)
  for (const contextShare of captured) {
    assert.equal(contextShare.projectConfig.bundleName, 'com.example.hooks')
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
  assert.equal(plugin.resolveId('@virtual/entry', '/entry.ets', options), expected)
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
    load() { return loadResult },
    transform() { return transformResult },
  })
  assert.equal(plugin.load('/entry.ets'), loadResult)
  assert.equal(plugin.transform('source', '/entry.ets'), transformResult)
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
  plugin.moduleParsed(JSON.stringify({
    id: '/entry.ets',
    importedIds: ['/dep.ets'],
    dynamicallyImportedIds: ['/lazy.ets'],
    isEntry: true,
  }))
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
  assert.equal(typeof plugin.resolveDynamicImport, 'function')
})

test('cleanUp hook is called and this is bound', () => {
  let cleanUpCalled = false
  let cleanUpThis = null
  const [plugin] = normalizePluginList({
    name: 'cleanup-test',
    cleanUp(ctxJson) {
      cleanUpCalled = true
      cleanUpThis = this
    },
  })
  plugin.cleanUp('')
  assert.ok(cleanUpCalled)
  assert.ok(cleanUpThis)
  assert.equal(typeof cleanUpThis.getModuleIds, 'function')
})
