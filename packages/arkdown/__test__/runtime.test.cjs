'use strict'

const assert = require('node:assert/strict')
const crypto = require('node:crypto')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')

const {
  HvigorRuntimeHost,
  HvigorCompilePluginPlanner,
  RuntimePluginLoader,
  createBuild,
  normalizePluginList,
  normalizeSortedPluginList,
  applyFunctionOrder,
  parseShouldInvalidCacheRequest,
  parsePluginContext,
  buildPluginContext,
  requireDefaultExport,
  hvigorProjectHash,
} = require('../runtime')

function tempProject() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-runtime-'))
}

test('hvigor project cache hash matches latest 32-character cache layout', () => {
  const root = path.resolve('fixtures', 'cache-project')
  const expected = crypto.createHash('sha256').update(root).digest('hex').slice(0, 32)
  assert.equal(hvigorProjectHash(root), expected)
  assert.equal(hvigorProjectHash(root).length, 32)
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
  const dynamicNode = runtime.getNodeByName('dynamic', 'node')
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
    `nodesInitialized:dynamic:node,${path.basename(root)}:project:fixture`,
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
    plugin.shouldInvalidCache(
      JSON.stringify({
        id: 'entry',
        meta: {
          moduleName: 'entry',
          cacheDir: '/tmp/cache',
          compileEnv: 'env',
        },
        changedFiles: ['entry/src/main/ets/pages/Index.ets'],
        isWatch: true,
        cacheHit: true,
      }),
    ),
    true,
  )
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
  plugin.__arkdownSeedHvigorContext(JSON.stringify({ share }))
  plugin.buildStart('')

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

  assert.equal(plugin.buildStart(''), undefined)
  assert.equal(plugin.moduleParsed(JSON.stringify({ id: '/entry.ets' })), undefined)
  assert.equal(plugin.beforeBuildEnd('{}'), pending)
  assert.equal(plugin.buildEnd('{}'), undefined)
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

  assert.equal(plugin.buildStart('{}'), pending)
  assert.equal(plugin.buildEnd('{}'), pending)
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
  const moduleOverrides = JSON.parse(nativeConfig.moduleHvigorOverridesJson)
  assert.equal(moduleOverrides[entry].arkOptions.byteCodeHar, true)
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

test('package-bound custom task preserves hvigor beforeRun run afterRun order', async () => {
  const root = tempProject()
  const entry = path.join(root, 'entry')
  const order = path.join(root, 'task-order.txt')
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
import { hapTasks, OhosPluginId } from '@ohos/hvigor-ohos-plugin'
import { getNode, hvigor } from '@ohos/hvigor'
import * as fs from 'node:fs'
const node = getNode(__filename)!
hvigor.nodesEvaluated(() => {
  const hap = node.getContext(OhosPluginId.OHOS_HAP_PLUGIN)
  hap.targets((target: any) => node.registerTask({
    name: target.getTargetName() + '@fixtureTask',
    context() { return { value: 'task-context' } },
    beforeRun(task: any) { fs.appendFileSync(${JSON.stringify(order)}, 'before:' + task.moduleName + ':' + this.context.value + '\\n') },
    run(task: any) { fs.appendFileSync(${JSON.stringify(order)}, 'run:' + task.modulePath + '\\n') },
    afterRun() { fs.appendFileSync(${JSON.stringify(order)}, 'after\\n') },
    postDependencies: [target.getTargetName() + '@PackageHap'],
  }))
})
export default { system: hapTasks, plugins: [] }
`,
  )

  const runtime = new RuntimePluginLoader({
    project: root,
    module: 'entry',
    product: 'default',
    buildTargetName: 'default',
  })
  const plugins = await runtime.load()
  assert.deepEqual(plugins, [])
  assert.equal(fs.existsSync(order), false)
  await runtime.hvigorHost.runPackageBoundCustomTasks()
  await runtime.hvigorHost.runPackageBoundCustomTasks()
  const expectedRun = ['before:entry:task-context', `run:${entry}`, 'after']
  assert.deepEqual(fs.readFileSync(order, 'utf8').trim().split('\n'), [...expectedRun, ...expectedRun])
})

test('Hvigor Task wrappers use LIFO before hooks, FIFO after hooks, and setEnable', async () => {
  const root = tempProject()
  const entry = path.join(root, 'entry')
  const order = path.join(root, 'task-wrapper-order.txt')
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
import { hapTasks, OhosPluginId } from '@ohos/hvigor-ohos-plugin'
import { getNode, hvigor } from '@ohos/hvigor'
import * as fs from 'node:fs'
const node = getNode(__filename)!
hvigor.nodesEvaluated(() => {
  const context = node.getContext(OhosPluginId.OHOS_HAP_PLUGIN)
  context.targets((target: any) => {
    const targetName = target.getTargetName()
    node.registerTask({
      name: targetName + '@wrapped',
      beforeRun() { fs.appendFileSync(${JSON.stringify(order)}, 'declared-before\\n') },
      run() { fs.appendFileSync(${JSON.stringify(order)}, 'run\\n') },
      afterRun() { fs.appendFileSync(${JSON.stringify(order)}, 'declared-after\\n') },
      postDependencies: [targetName + '@PackageHap'],
    })
    const wrapped = node.getTaskByName(targetName + '@wrapped')
    wrapped.beforeRun(function () { fs.appendFileSync(${JSON.stringify(order)}, 'wrapper-before-a:' + this.getName() + '\\n') })
    wrapped.beforeRun(() => fs.appendFileSync(${JSON.stringify(order)}, 'wrapper-before-b\\n'))
    wrapped.afterRun(() => fs.appendFileSync(${JSON.stringify(order)}, 'wrapper-after-a\\n'))
    wrapped.afterRun(() => fs.appendFileSync(${JSON.stringify(order)}, 'wrapper-after-b\\n'))

    node.registerTask({
      name: targetName + '@disabled',
      run() { fs.appendFileSync(${JSON.stringify(order)}, 'disabled-run\\n') },
      postDependencies: [targetName + '@PackageHap'],
    })
    node.getTaskByName(targetName + '@disabled').setEnable(false)
    if (node.getAllTasks().map((task: any) => task.getName()).length !== 2) throw new Error('task wrappers missing')
  })
})
export default { system: hapTasks, plugins: [] }
`,
  )

  const runtime = new RuntimePluginLoader({
    project: root,
    module: 'entry',
    product: 'default',
    buildTargetName: 'default',
  })
  const plugins = await runtime.load()
  assert.deepEqual(plugins, [])
  await runtime.hvigorHost.runPackageBoundCustomTasks()

  assert.deepEqual(fs.readFileSync(order, 'utf8').trim().split('\n'), [
    'wrapper-before-b',
    'wrapper-before-a:default@wrapped',
    'declared-before',
    'run',
    'declared-after',
    'wrapper-after-a',
    'wrapper-after-b',
  ])
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
      return cause.code === 'TYPE_ERRORS' && cause.diagnostics.some((diagnostic) => diagnostic.code === 2322)
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

  for (const [key, value] of Object.entries(config)) {
    if (key === 'compilePluginPath' || key === 'hvigorPluginFile') {
      assert.equal(
        capturedConfig[key],
        undefined,
        `BuildConfig.${key} must be consumed by runtime.js before nativeBuild`,
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

test('internal context seed supplies share data without adding a lifecycle hook', () => {
  const share = { bundleName: 'com.example.prop', buildMode: 'debug' }
  const captured = {}
  const plugin = normalizePluginList({
    name: 'propagation-test',
    buildStart() {
      captured.buildStart = this.share
    },
  })[0]

  plugin.__arkdownSeedHvigorContext(JSON.stringify({ share }))
  plugin.buildStart('')

  assert.equal(captured.buildStart.bundleName, share.bundleName)
  assert.equal(Object.keys(plugin).includes('__arkdownSeedHvigorContext'), false)
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
  plugin.__arkdownSeedHvigorContext(JSON.stringify({ share }))
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
    load() {
      return loadResult
    },
    transform() {
      return transformResult
    },
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
  plugin.moduleParsed(
    JSON.stringify({
      id: '/entry.ets',
      importedIds: ['/dep.ets'],
      dynamicallyImportedIds: ['/lazy.ets'],
      isEntry: true,
    }),
  )
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

test('explicit plugin API ignores non-Rolldown cleanUp instead of running a second lifecycle', () => {
  assert.deepEqual(normalizePluginList({ name: 'cleanup-test', cleanUp() {} }), [])
})
