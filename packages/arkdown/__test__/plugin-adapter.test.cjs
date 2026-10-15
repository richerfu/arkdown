'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')

// These ABI unit tests inject a binding. Loading the adapter must not require a
// locally built addon; real binding coverage belongs to the native E2E suite.
const bindingPath = require.resolve('../index.js')
const previousBinding = require.cache[bindingPath]
require.cache[bindingPath] = {
  id: bindingPath,
  filename: bindingPath,
  loaded: true,
  exports: fakeBinding(),
}
const {
  COMPILE_PLUGIN_HOOKS,
  ESMODULE_DORMANT_HOOKS,
  PLUGIN_HOOKS,
  PluginAdapter,
  PluginModuleLoader,
  createSdkPluginDispatch,
  createSdkPluginHost,
  normalizePlugin,
  normalizePluginList,
  parsePluginAst,
} = require('../js/plugin-adapter.js')
if (previousBinding) require.cache[bindingPath] = previousBinding
else delete require.cache[bindingPath]

function fakeBinding() {
  return {
    compilePluginModule: async (file) =>
      `module.exports = { default: { name: ${JSON.stringify(path.basename(file))}, transform(code) { return code } } }`,
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

test('SDK checker plugins run in the build-scoped JavaScript compatibility host', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-sdk-plugin-'))
  const typescriptRoot = path.join(root, 'node_modules', 'typescript')
  fs.mkdirSync(typescriptRoot, { recursive: true })
  fs.writeFileSync(
    path.join(typescriptRoot, 'index.js'),
    `let sourceFileCreations = 0
exports.ScriptKind = { TS: 3, ETS: 99 }
exports.ScriptTarget = { Latest: 99 }
exports.createSourceFile = (fileName, source) => {
  sourceFileCreations += 1
  return ({
  fileName, source, pos: 0, end: source.length,
  getStart() { return 0 }, getText() { return source }
  })
}
exports.sourceFileCreations = () => sourceFileCreations
exports.forEachChild = () => {}`,
  )
  const plugin = path.join(root, 'checker.cjs')
  fs.writeFileSync(
    plugin,
    `let constructions = 0
exports.value = (required, target, scene) => ({ result: required === '11' && target === '17' && scene === 0, message: 'value' })
exports.regex = () => /^(\\d+)\\.(\\d+)$/
exports.SyscapChecker = class {
  constructor() { constructions += 1 }
  check(node, declaration, config) {
    return { checkResult: constructions === 1 && node.getText() === 'current' &&
      declaration.getText() === 'declare current' && config.syscapIntersectionSet.has('base') &&
      config.syscapUnionSet.has('extension'), checkMessage: 'syscap' }
  }
}`,
  )
  const invoke = createSdkPluginHost()
  assert.deepEqual(
    invoke({
      sessionId: 7,
      pluginId: 1,
      path: plugin,
      functionName: 'value',
      operation: 'value',
      args: ['11', '17', 0],
    }),
    {
      found: true,
      result: true,
      message: 'value',
    },
  )
  assert.deepEqual(
    invoke({ sessionId: 7, pluginId: 2, path: plugin, functionName: 'regex', operation: 'regex', args: ['5.17'] }),
    {
      found: true,
      matched: true,
      groups: ['5.17', '5', '17'],
    },
  )
  assert.deepEqual(invoke({ sessionId: 7, pluginId: 1, operation: 'value', args: ['11', '17', 0] }), {
    found: true,
    result: true,
    message: 'value',
  })
  const classDescriptor = { path: plugin, functionName: 'SyscapChecker', className: 'SyscapChecker' }
  const classRequest = { sessionId: 7, pluginId: 3 }
  assert.deepEqual(invoke({ ...classRequest, ...classDescriptor, operation: 'prepareClass', args: [] }), {
    found: true,
  })
  assert.deepEqual(invoke({ ...classRequest, operation: 'prepareClass', args: [] }), { found: true })
  const syscapRequest = {
    ...classRequest,
    sessionId: 7,
    operation: 'syscap',
    args: [
      {
        node: { sourceId: 1, fileName: 'input.ets', source: 'current', pos: 0, end: 7, start: 0 },
        declaration: {
          sourceId: 2,
          fileName: 'api.d.ets',
          source: 'declare current',
          pos: 0,
          end: 15,
          start: 0,
        },
        projectConfigId: 1,
        projectConfig: {
          etsLoaderPath: root,
          syscapIntersectionSet: ['base'],
          syscapUnionSet: ['extension'],
        },
      },
    ],
  }
  assert.deepEqual(invoke(syscapRequest), { found: true, checkResult: true, checkMessage: 'syscap' })
  const unmatchedNode = invoke({
    ...syscapRequest,
    args: [
      {
        ...syscapRequest.args[0],
        node: { sourceId: 1, fileName: 'input.ets', pos: 1, end: 7, start: 1 },
      },
    ],
  })
  assert.equal(unmatchedNode.found, true)
  assert.equal(unmatchedNode.phase, 'invoke')
  assert.match(unmatchedNode.error, /Unable to recreate SDK checker source node/)
  assert.deepEqual(
    invoke({
      ...syscapRequest,
      args: [
        {
          node: { sourceId: 1, fileName: 'input.ets', pos: 0, end: 7, start: 0 },
          declaration: {
            sourceId: 2,
            fileName: 'api.d.ets',
            pos: 0,
            end: 15,
            start: 0,
          },
          projectConfigId: 1,
        },
      ],
    }),
    { found: true, checkResult: true, checkMessage: 'syscap' },
  )
  assert.equal(require(typescriptRoot).sourceFileCreations(), 2)
  const otherSession = { ...syscapRequest, ...classDescriptor, sessionId: 8 }
  invoke(otherSession)
  const creationsBeforeRelease = require(typescriptRoot).sourceFileCreations()
  const gcCalls = []
  const previousGc = Object.getOwnPropertyDescriptor(global, 'gc')
  const previousNodeVersion = Object.getOwnPropertyDescriptor(process.versions, 'node')
  t.after(() => {
    if (previousGc) Object.defineProperty(global, 'gc', previousGc)
    else delete global.gc
    Object.defineProperty(process.versions, 'node', previousNodeVersion)
  })
  Object.defineProperty(process.versions, 'node', { ...previousNodeVersion, value: '22.0.0' })
  global.gc = (...args) => gcCalls.push(args)
  const { stats, ...released } = invoke({ sessionId: 7, operation: 'releaseSession', args: [] })
  assert.deepEqual(released, { found: true })
  assert.deepEqual(
    gcCalls,
    [[{ type: 'major', execution: 'sync', flavor: 'last-resort' }]],
    'session release requests page reclamation before the next build stage',
  )
  invoke({ sessionId: 7, operation: 'releaseSession', args: [] })
  assert.equal(gcCalls.length, 1, 'an already released session does not collect again')
  assert.equal(stats.parses, 2)
  assert.equal(stats.reparses, 0)
  assert.equal(stats.batches, 3)
  assert.ok(stats.peakCommitted > 0 && stats.retainedCommitted > 0)
  const otherReferences = {
    ...classRequest,
    sessionId: 8,
    args: [
      {
        node: { ...syscapRequest.args[0].node, source: undefined },
        declaration: { ...syscapRequest.args[0].declaration, source: undefined },
        projectConfigId: 1,
      },
    ],
  }
  invoke(otherReferences)
  assert.equal(
    require(typescriptRoot).sourceFileCreations(),
    creationsBeforeRelease,
    'releasing one session must preserve another session sharing its file names',
  )
  const expired = invoke({ ...otherReferences, sessionId: 7 })
  assert.equal(expired.phase, 'load')
  assert.match(expired.error, /not registered/)
  invoke({ ...syscapRequest, ...classDescriptor })
  assert.equal(
    require(typescriptRoot).sourceFileCreations(),
    creationsBeforeRelease + 2,
    'released ASTs must be recreated from a newly registered source',
  )
  for (const [version, sessionId] of [
    ['18.17.0', 8],
    ['20.0.0', 7],
  ]) {
    Object.defineProperty(process.versions, 'node', { ...previousNodeVersion, value: version })
    invoke({ sessionId, operation: 'releaseSession', args: [] })
    assert.deepEqual(gcCalls.at(-1), [], `Node ${version} release retains no-argument full GC`)
  }
  assert.equal(gcCalls.length, 3, 'both legacy sessions still trigger a full collection')

  assert.deepEqual(
    JSON.parse(
      createSdkPluginDispatch()(
        JSON.stringify([
          { sessionId: 7, pluginId: 1, path: plugin, functionName: 'value', operation: 'value', args: ['11', '17', 0] },
          { sessionId: 7, pluginId: 2, path: plugin, functionName: 'regex', operation: 'regex', args: ['5.17'] },
        ]),
      ),
    ),
    [
      { found: true, result: true, message: 'value' },
      { found: true, matched: true, groups: ['5.17', '5', '17'] },
    ],
  )
  assert.equal(
    invoke({
      sessionId: 7,
      pluginId: 4,
      path: path.join(root, 'missing.cjs'),
      functionName: 'value',
      operation: 'value',
      args: [],
    }).phase,
    'load',
  )
})

// Observe the stored representation without exposing production host internals.
function trackSnapshotBuffers(t, text) {
  const buffers = [],
    decoded = []
  const from = Buffer.from,
    toString = Buffer.prototype.toString
  t.mock.method(Buffer, 'from', function (value, ...args) {
    const buffer = Reflect.apply(from, Buffer, [value, ...args])
    if (value === text) buffers.push(buffer)
    return buffer
  })
  t.mock.method(Buffer.prototype, 'toString', function (...args) {
    const result = Reflect.apply(toString, this, args)
    if (buffers.includes(this)) decoded.push(result)
    return result
  })
  return { buffers, decoded }
}

test('SDK checker AST LRU reparses evicted sources and retains declarations and recent hits', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-sdk-lru-'))
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))
  const typescriptRoot = path.join(root, 'node_modules', 'typescript')
  fs.mkdirSync(typescriptRoot, { recursive: true })
  fs.writeFileSync(
    path.join(typescriptRoot, 'index.js'),
    `
const creations = new Map()
exports.creations = creations
exports.ScriptKind = { TS: 3, ETS: 99 }
exports.ScriptTarget = { Latest: 99 }
exports.createSourceFile = (fileName, text, target, setParentNodes, scriptKind) => {
  if (!setParentNodes) throw new Error('SDK nodes require parents')
  creations.set(fileName, (creations.get(fileName) || 0) + 1)
  const sourceFile = { fileName, text, pos: 0, end: text.length, getStart() { return 0 }, getText() { return text } }
  const start = text.indexOf('current')
  sourceFile.child = {
    pos: 0, end: text.length, parent: sourceFile,
    getStart() { return start },
    getText() { return text.slice(start) },
    getSourceFile() { return sourceFile },
  }
  return sourceFile
}
exports.forEachChild = (node, visit) => { if (node.child) visit(node.child) }
`,
  )
  const plugin = path.join(root, 'checker.cjs')
  fs.writeFileSync(
    plugin,
    `
exports.Checker = class {
  check(node, declaration) {
    const sf = node.getSourceFile()
    if (node.parent !== sf || declaration.parent !== declaration.getSourceFile()) throw new Error('lost parents')
    return { checkResult: true, checkMessage: sf.fileName + ':' + node.getStart() + ':' + node.end + ':' + node.getText() }
  }
}
`,
  )
  const invoke = createSdkPluginHost()
  const descriptor = (sourceId, fileName, source = '/* 中文 😀 */ current') => ({
    sourceId,
    fileName,
    source,
    pos: 0,
    end: source.length,
    start: source.indexOf('current'),
  })
  const reference = (snapshot) => ({ ...snapshot, source: undefined })
  const declaration = descriptor(1000, 'api.d.ets')
  const tsDeclaration = descriptor(1001, 'other.d.ts')
  const app = (index) => descriptor(index, `input${index}.ets`)
  const request = (node, decl = reference(declaration), config) =>
    invoke({
      sessionId: 1,
      pluginId: 1,
      ...(config ? { path: plugin, functionName: 'Checker', className: 'Checker' } : {}),
      operation: 'syscap',
      args: [{ node, declaration: decl, projectConfigId: 1, ...(config ? { projectConfig: config } : {}) }],
    })
  const { buffers, decoded } = trackSnapshotBuffers(t, app(2).source)
  const first = request(app(1), declaration, { etsLoaderPath: root })
  assert.equal(first.checkResult, true)
  request(app(2), tsDeclaration)
  for (let index = 3; index <= 64; index += 1) assert.equal(request(app(index)).checkResult, true)
  // A hit refreshes recency: adding file 65 must evict file 2, not file 1.
  assert.deepEqual(request(reference(app(1))), first)
  request(app(65))
  const { creations } = require(typescriptRoot)
  assert.equal(creations.get('input1.ets'), 1)
  assert.equal(creations.get('input2.ets'), 1)
  assert.equal(buffers.length, 1, 'LRU eviction encodes the evicted application snapshot')
  assert.equal(buffers[0].length, Buffer.byteLength(app(2).source))
  const second = request(reference(app(2)))
  assert.deepEqual(decoded, [app(2).source], 'decode UTF-8 once before reparsing at UTF-16 offsets')
  assert.equal(second.checkMessage, `input2.ets:${app(2).start}:${app(2).end}:current`)
  assert.equal(creations.get('input2.ets'), 2, 'evicted AST must be parsed from retained source text')
  assert.deepEqual(request(reference(app(2))), second)
  assert.equal(creations.get('input2.ets'), 2, 'reparsed nodes must be cached again')
  request(reference(app(2)), reference(tsDeclaration))
  assert.equal(creations.get('api.d.ets'), 1)
  assert.equal(creations.get('other.d.ts'), 1, '.d.ts stays resident despite no hits during LRU churn')
  const mismatch = request({ ...reference(app(2)), start: app(2).start + 1 })
  assert.equal(mismatch.phase, 'invoke')
  assert.match(mismatch.error, /Unable to recreate SDK checker source node/)
  const legacy = request({ ...reference(app(2)), start: 0, text: 'current' })
  assert.equal(legacy.checkResult, true, 'legacy text overrides a nonmatching start')
  const wrongText = request({ ...reference(app(2)), start: 0, text: 'changed' })
  assert.equal(wrongText.phase, 'invoke', 'cached spans must still compare exceptional legacy text')
  const updated = descriptor(2000, 'input2.ets', '/* 😀 */ currentChanged')
  assert.equal(request(updated).checkMessage, `input2.ets:${updated.start}:${updated.end}:currentChanged`)
  assert.deepEqual(request(reference(app(2))), second, 'old and updated source IDs must stay distinct')
})

function sdkBatchFixture(t, prefix = '') {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-sdk-batch-'))
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))
  const tsRoot = path.join(root, 'node_modules', 'typescript')
  fs.mkdirSync(tsRoot, { recursive: true })
  fs.writeFileSync(
    path.join(tsRoot, 'index.js'),
    `
exports.creations = new Map()
exports.starts = 0
exports.ScriptKind = { TS: 3, ETS: 99 }
exports.ScriptTarget = { Latest: 99 }
exports.createSourceFile = (fileName, text, target, parents) => {
  if (!parents) throw new Error('SDK nodes require parents')
  if (typeof text !== 'string') throw new Error('parser needs a decoded string')
  exports.creations.set(fileName, (exports.creations.get(fileName) || 0) + 1)
  const sf = { fileName, pos: 0, end: text.length,
    getStart() { throw new Error('getStart on a non-candidate ancestor') } }
  const start = text.indexOf('name')
  sf.child = { pos: start, end: start + 4, parent: sf,
    getStart() { exports.starts++; return start },
    getText() { return text.slice(start, start + 4) }, getSourceFile() { return sf } }
  return sf
}
exports.forEachChild = (node, visit) => { if (node.child) visit(node.child) }
`,
  )
  const plugin = path.join(root, 'checker.cjs')
  fs.writeFileSync(
    plugin,
    `
exports.calls = []
exports.nodes = []
exports.value = () => ({ result: true, message: 'value' })
exports.format = () => ({ result: true, message: 'format' })
exports.configs = []
exports.instances = 0
exports.Checker = class {
  constructor() { exports.instances++ }
  check(node, declaration, config) {
    exports.configs.push(config)
    exports.nodes.push([node.pos, node.end, node.getText(), declaration.pos, declaration.end, declaration.getText()])
    if (!node.parent || !declaration.parent) throw new Error('lost parent')
    const name = node.getSourceFile().fileName
    exports.calls.push(name)
    return { checkResult: true, checkMessage: name }
  }
}
exports.Gate = class {
  check() { return { checkResult: false } }
}
exports.Throws = class {
  check() { throw new Error('SDK failure') }
}
`,
  )
  const dispatch = createSdkPluginDispatch()
  const invoke = (requests) => JSON.parse(dispatch(JSON.stringify(requests)))
  const snapshot = (id, fileName, full = false) => ({
    sourceId: id,
    fileName,
    pos: prefix.length,
    end: prefix.length + 4,
    start: prefix.length,
    ...(full ? { source: prefix + 'name; // trailing comment' } : {}),
  })
  const request = (i, full = false) => ({
    sessionId: 1,
    pluginId: 1,
    operation: 'syscap',
    ...(full ? { path: plugin, functionName: 'Checker', className: 'Checker' } : {}),
    args: [
      {
        node: snapshot(i, `file${i}.ets`, full),
        declaration: snapshot(1000, 'api.d.ets', full),
        projectConfigId: 1,
        ...(full ? { projectConfig: { etsLoaderPath: root } } : {}),
      },
    ],
  })
  return {
    invoke,
    request,
    plugin,
    ts: require(tsRoot),
    calls: require(plugin).calls,
    configs: require(plugin).configs,
    nodes: require(plugin).nodes,
  }
}

test('SDK compaction reparses application and declaration nodes from retained source IDs', (t) => {
  const previousGc = Object.getOwnPropertyDescriptor(global, 'gc')
  const previousVersion = Object.getOwnPropertyDescriptor(process.versions, 'node')
  t.after(() => {
    if (previousGc) Object.defineProperty(global, 'gc', previousGc)
    else delete global.gc
    Object.defineProperty(process.versions, 'node', previousVersion)
  })
  t.mock.method(process, 'memoryUsage', () => ({ heapUsed: 40 << 20 }))
  const prefix = '/* 中文 😀 */ '
  const text = prefix + 'name; // trailing comment'
  const { buffers, decoded } = trackSnapshotBuffers(t, text)
  for (const version of ['22.0.0', '18.17.0', '20.0.0']) {
    Object.defineProperty(process.versions, 'node', { ...previousVersion, value: version })
    const gcCalls = []
    global.gc = (...args) => gcCalls.push(args)
    const { invoke, request, plugin, ts, configs, nodes } = sdkBatchFixture(t, prefix)
    const encodedBefore = buffers.length,
      decodedBefore = decoded.length
    assert.deepEqual(invoke([{ sessionId: 1, operation: 'compactSession' }]), [{ found: true }])
    assert.deepEqual(gcCalls, [], 'an unknown session needs neither allocation nor collection')
    const original = invoke([request(1, true)])
    const [{ stats }] = invoke([{ sessionId: 1, operation: 'compactSession' }])
    assert.equal(buffers.length - encodedBefore, 2, 'compact encodes application and declaration snapshots')
    assert.deepEqual([stats.parses, stats.reparses, stats.batches], [2, 0, 1])
    assert.ok(stats.committedBefore > 0 && stats.retainedCommitted > 0)
    assert.deepEqual(
      gcCalls,
      version.startsWith('22') ? [[{ type: 'major', execution: 'sync', flavor: 'last-resort' }]] : [[]],
    )
    assert.deepEqual(invoke([request(1)]), original, 'sourceId-only callbacks retain node identity and text')
    assert.deepEqual(decoded.slice(decodedBefore), [text, text])
    assert.deepEqual(
      nodes,
      Array(2).fill([prefix.length, prefix.length + 4, 'name', prefix.length, prefix.length + 4, 'name']),
    )
    assert.equal(buffers.length - encodedBefore, 2, 'reparsing stores strings without encoding another copy')
    assert.equal(ts.creations.get('file1.ets'), 2)
    assert.equal(ts.creations.get('api.d.ets'), 2, 'declaration ASTs must also be rebuilt')
    assert.strictEqual(configs[1], configs[0], 'the registered project config survives compaction')
    assert.equal(require(plugin).instances, 1, 'the SDK class instance survives compaction')
    const [released] = invoke([{ sessionId: 1, operation: 'releaseSession' }])
    assert.deepEqual([released.stats.parses, released.stats.reparses, released.stats.batches], [4, 2, 2])
    assert.deepEqual(gcCalls[1], gcCalls[0], 'release and compaction use the same reduce-memory policy')
  }
})

test('SDK source release rejects later snapshots but retains function plugins until session release', (t) => {
  const { invoke, request, plugin } = sdkBatchFixture(t)
  assert.deepEqual(invoke([{ sessionId: 1, operation: 'releaseSources' }]), [{ found: true }])
  const functions = ['value', 'format'].map((operation, index) => ({
    sessionId: 1,
    pluginId: index + 2,
    operation,
    args: [],
  }))
  const original = invoke(functions.map((req) => ({ ...req, path: plugin, functionName: req.operation })))
  assert.ok(invoke([request(1, true)])[0].checkResult)
  const [{ stats }] = invoke([{ sessionId: 1, operation: 'releaseSources' }])
  assert.deepEqual([stats.parses, stats.reparses], [2, 0])
  // Even a repeated full publication must not revive the released snapshots.
  for (const full of [false, true]) {
    const [failed] = invoke([request(1, full)])
    assert.equal(failed.phase, 'invoke')
    assert.match(failed.error, /SDK checker source 1:1: sources were released before strict diagnostics/)
  }
  assert.deepEqual(invoke(functions), original, 'registrations survive source release')
  assert.deepEqual(
    original.map((r) => [r.result, r.message]),
    [
      [true, 'value'],
      [true, 'format'],
    ],
  )
  assert.equal(invoke([{ sessionId: 1, operation: 'compactSession' }])[0].found, true)
  assert.equal(invoke([request(1)])[0].phase, 'invoke', 'compaction must not revive released sources')
  assert.equal(invoke([{ sessionId: 1, operation: 'releaseSession' }])[0].found, true)
  assert.deepEqual(invoke([{ sessionId: 1, operation: 'releaseSession' }]), [{ found: true }])
})

test('syscap batches group source IDs, prune before getStart, and retain the 64-entry LRU', (t) => {
  const { invoke, request, ts, calls } = sdkBatchFixture(t)
  const batch = Array.from({ length: 70 }, (_, i) => request(i, true)).concat(
    Array.from({ length: 70 }, (_, i) => request(i)),
  )
  const responses = invoke(batch)
  const names = batch.map((r) => r.args[0].node.fileName)
  assert.deepEqual(
    responses.map((r) => r.checkMessage),
    names,
  )
  assert.deepEqual(calls, names, 'source grouping must preserve callback order')
  assert.equal(ts.creations.size, 71)
  assert.ok(
    [...ts.creations.values()].every((count) => count === 1),
    'interleaved sources parse once per batch',
  )
  assert.equal(ts.starts, batch.length * 2, 'only the end-matching node and declaration candidates call getStart')
  assert.equal(invoke([request(6)])[0].checkResult, true)
  assert.equal(ts.creations.get('file6.ets'), 1, 'the newest 64 application ASTs remain resident')
  assert.equal(invoke([request(5)])[0].checkResult, true)
  assert.equal(ts.creations.get('file5.ets'), 2, 'the 65th most recent application AST was evicted')
  assert.equal(ts.creations.get('api.d.ets'), 1, 'declarations are outside the application LRU')
  const [{ stats }] = invoke([{ sessionId: 1, operation: 'releaseSession', args: [] }])
  assert.equal(stats.parses, 72)
  assert.equal(stats.reparses, 1)
  assert.equal(stats.batches, 3)
})

test('SDK batches collect only above 128 MiB and twice the last post-GC heap', (t) => {
  const mb = 1 << 20
  let used = 40 * mb
  let afterGc = 70 * mb
  const collectedAfter = []
  const previousGc = Object.getOwnPropertyDescriptor(global, 'gc')
  t.after(() => {
    if (previousGc) Object.defineProperty(global, 'gc', previousGc)
    else delete global.gc
  })
  t.mock.method(process, 'memoryUsage', () => ({ heapUsed: used }))
  const { invoke, request, calls } = sdkBatchFixture(t)
  global.gc = (...args) => {
    assert.deepEqual(args, [], 'repeated batch-threshold collections stay plain')
    collectedAfter.push(calls.length)
    used = afterGc
  }
  const run = (heap) => {
    used = heap * mb
    const count = calls.length
    const responses = invoke([request(0, count === 0), request(0)])
    assert.deepEqual(
      responses.map((r) => r.checkResult),
      [true, true],
    )
    assert.equal(calls.length, count + 2)
  }
  run(100) // More than 2x the initial heap, below the floor.
  run(128) // The floor is exclusive.
  assert.deepEqual(collectedAfter, [])
  run(129)
  assert.deepEqual(collectedAfter, [6], 'collect after every callback in the batch')
  run(140) // Exactly 2x the last post-GC value: no collection.
  assert.deepEqual(collectedAfter, [6])
  afterGc = 90 * mb
  run(141)
  assert.deepEqual(collectedAfter, [6, 10])
  run(180)
  assert.deepEqual(collectedAfter, [6, 10], 'update the baseline after each GC')
  global.gc = undefined
  run(300) // Embedders without --expose-gc still dispatch normally.
  assert.deepEqual(collectedAfter, [6, 10])
})

test('syscap batch dependencies short-circuit callbacks but publish skipped descriptors and sources', (t) => {
  const { invoke, request, plugin, calls } = sdkBatchFixture(t)
  const descriptor = (pluginId, className) => ({ pluginId, path: plugin, functionName: className, className })
  const first = request(0, true)
  const skipped = { ...request(1, true), ...descriptor(2, 'Checker'), after: 0 }
  const result = invoke([{ ...first, ...descriptor(1, 'Gate') }, skipped])
  assert.equal(result[0].checkResult, false)
  assert.deepEqual(result[1], { skipped: true })
  assert.deepEqual(calls, [])
  assert.equal(
    invoke([{ ...request(1), pluginId: 2 }])[0].checkMessage,
    'file1.ets',
    'a later batch can reference sources and descriptors first sent on a skipped request',
  )
  const chain = invoke([
    { ...request(0), ...descriptor(3, 'Throws') },
    { ...request(1), pluginId: 2, after: 0 },
    { ...request(1), pluginId: 2, after: 1 },
    { ...request(0), ...descriptor(4, 'Missing'), path: plugin + '.missing' },
    { ...request(1), pluginId: 2, after: 3 },
  ])
  assert.equal(chain[0].phase, 'invoke')
  assert.deepEqual(chain[1], { skipped: true })
  assert.deepEqual(chain[2], { skipped: true })
  assert.equal(chain[3].phase, 'load')
  assert.equal(chain[4].checkResult, true, 'load failures preserve the existing skip-missing-plugin behavior')
  assert.deepEqual(calls, ['file1.ets', 'file1.ets'])
})

test('plugin parse follows Rollup 4.23 ParseAst options and error shape', () => {
  let observedOptions
  const binding = {
    parsePluginAst(_source, options) {
      observedOptions = options
      return {
        ast: {
          node: { type: 'Program', start: 0, end: 8, body: [], sourceType: 'module' },
          fixes: [],
        },
      }
    },
  }
  const comments = []
  const ast = parsePluginAst(
    'return 1',
    {
      allowReturnOutsideFunction: true,
      sourceType: 'script',
      ranges: true,
      locations: true,
      onComment: comments,
      onToken: [],
    },
    null,
    binding,
  )
  assert.deepEqual(observedOptions, { allowReturnOutsideFunction: true })
  assert.equal(ast.sourceType, 'module')
  assert.deepEqual(comments, [])

  assert.throws(
    () =>
      parsePluginAst('return 1', {}, null, {
        parsePluginAst() {
          throw new Error('PLUGIN_PARSE_ERROR: Return statement is not allowed here (1:0) [pos=0]')
        },
      }),
    (error) => error.name === 'RollupError' && error.code === 'PARSE_ERROR' && error.pos === 0 && !error.loc,
  )
})

test('explicit Rolldown plugins preserve the complete Arkdown hook surface', async () => {
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
  assert.deepEqual(await dispatch(normalized, 'buildStart', { buildStartOptions: { input: 'index.ets' } }), {})
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
  assert.deepEqual(dispatch(normalized, 'watchChange', { id: '/project/Index.ets', watchEvent: 'update' }), {})
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
  for (const hookName of ESMODULE_DORMANT_HOOKS) {
    assert.deepEqual(
      await dispatch(normalized, hookName, {
        code: 'export {}',
        chunk: { fileName: 'entry.js' },
        outputOptions: {},
        outputHookOptions: {},
        bundle: {},
      }),
      {},
    )
  }
  assert.equal(calls.length, PLUGIN_HOOKS.length)
  assert.equal(
    calls.every((call) => call.share === share),
    true,
  )
  assert.deepEqual(calls.find((call) => call.hookName === 'watchChange').args, [
    '/project/Index.ets',
    { event: 'update' },
  ])
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
      { pluginFile: '/plugins/persistent.ts' },
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
  assert.deepEqual(calls, [{ name: 'entry', id: 'Index.ets' }, { name: 'beforeBuildEnd' }])
})

test('explicit plugin context delegates graph operations to Rolldown and retains cache/async state', async () => {
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
        this.async(() =>
          setImmediate(() => {
            calls.push(['async-complete'])
            pluginContext.signal()
          }),
        )
        calls.push(['transformContext', id, options, this.getCombinedSourcemap()])
        return code
      },
    },
    fakeBinding(),
    {},
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

test('module metadata returned by plugins is merged for later hooks and context loads', async () => {
  const nativeModule = { id: '/project/index.ts', meta: {}, moduleSideEffects: null }
  const nativeContext = {
    getModuleInfo: () => nativeModule,
    load: async () => nativeModule,
  }
  const producer = normalizePlugin(
    {
      name: 'metadata-producer',
      transform(code) {
        return { code, meta: { producer: () => 'live' }, moduleSideEffects: false }
      },
    },
    fakeBinding(),
  )
  let observed
  const consumer = normalizePlugin(
    {
      name: 'metadata-consumer',
      async transform(code, id) {
        const loaded = await this.load({ id, meta: { loaded: true }, moduleSideEffects: 'no-treeshake' })
        const info = this.getModuleInfo(id)
        observed = { loaded, info }
        return { code, meta: { consumer: true } }
      },
      moduleParsed(info) {
        observed.parsed = info
      },
    },
    fakeBinding(),
  )
  const share = { __arkdownBuildInstanceId: 'metadata-build', projectConfig: {} }
  await dispatch(producer, 'seedContext', { share })
  await dispatch(consumer, 'seedContext', { share })

  const produced = await dispatch(producer, 'transform', {
    context: nativeContext,
    id: nativeModule.id,
    code: 'export {}',
    transformOptions: { moduleType: 'ts' },
  })
  assert.deepEqual(produced.transform, { code: 'export {}', moduleSideEffects: false })
  await dispatch(consumer, 'transform', {
    context: nativeContext,
    id: nativeModule.id,
    code: 'export {}',
    transformOptions: { moduleType: 'ts' },
  })
  await dispatch(consumer, 'moduleParsed', { context: nativeContext, moduleInfo: nativeModule })

  assert.equal(observed.loaded.meta.producer(), 'live')
  assert.equal(observed.loaded.meta.loaded, true)
  assert.equal(observed.loaded.moduleSideEffects, 'no-treeshake')
  assert.equal(observed.info.meta.producer(), 'live')
  assert.equal(observed.info.meta.loaded, true)
  assert.equal(observed.parsed.meta.consumer, true)
  assert.equal(observed.parsed.moduleSideEffects, 'no-treeshake')
})

test('explicit plugin factories and nested arrays retain deterministic native order', () => {
  const binding = fakeBinding()
  binding.planRolldownPackagePlugins = () => ({
    placements: [1, 0].map((pluginIndex) => ({ pluginIndex })),
  })
  const plugins = normalizePluginList(
    () => [{ name: 'last', transform: (code) => code }, [{ name: 'first', load: () => null }]],
    binding,
  )

  assert.deepEqual(
    plugins.map((plugin) => plugin.name),
    ['first', 'last'],
  )
})

test('configured CJS plugin modules use default exports and only the Node executable reaches Rust', async (context) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-plugin-adapter-'))
  const pluginFile = path.join(root, 'compile-plugin.cjs')
  fs.writeFileSync(pluginFile, "exports.default = { name: 'configured', transform(code) { return code } }\n")
  context.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const prepared = await new PluginAdapter(fakeBinding()).prepare({
    project: root,
    target: 'hap',
    compilePluginPath: './compile-plugin.cjs',
  })

  assert.deepEqual(prepared.config, {
    project: root,
    target: 'hap',
    nodeExecutable: process.execPath,
  })
  assert.equal(prepared.plugins.length, 1)
  assert.equal(prepared.plugins[0].name, 'configured')
  assert.equal(prepared.plugins[0].__arkdownHvigorCompilePlugin, true)
})

test('configured Hvigor plugins expose only the source-owned hook and context ABI', async (context) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-hvigor-plugin-abi-'))
  const pluginFile = path.join(root, 'compile-plugin.cjs')
  fs.writeFileSync(
    pluginFile,
    `exports.default = {
      name: 'source-abi',
      buildStart() { globalThis.__arkdownHvigorBuildStart = [arguments.length, Object.keys(this).sort()] },
      transform(code, id) { globalThis.__arkdownHvigorTransform = [arguments.length, code, id]; return code },
      beforeBuild() { throw new Error('unknown hooks must not be wrapped') }
    }\n`,
  )
  context.after(() => {
    delete globalThis.__arkdownHvigorBuildStart
    delete globalThis.__arkdownHvigorTransform
    fs.rmSync(root, { recursive: true, force: true })
  })

  const binding = fakeBinding()
  binding.validateHvigorCompilePlugins = () => ({
    warnings: [{ code: 'UNKNOWN_PLUGIN_ATTR', message: 'beforeBuild is not a compile-plugin hook' }],
  })
  const prepared = await new PluginAdapter(binding).prepare({
    project: root,
    compilePluginPath: './compile-plugin.cjs',
  })
  const plugin = prepared.plugins[0]
  await dispatch(plugin, 'seedContext', { share: { projectConfig: {} } })
  await dispatch(plugin, 'buildStart', { buildStartOptions: { input: 'Index.ets' } })
  await dispatch(plugin, 'transform', {
    code: 'export {}',
    id: 'Index.ets',
    transformOptions: { moduleType: 'ts' },
  })

  assert.deepEqual(plugin.__arkdownPluginHooks, ['buildStart', 'transform'])
  assert.deepEqual(COMPILE_PLUGIN_HOOKS, [
    'buildStart',
    'resolveId',
    'load',
    'transform',
    'shouldInvalidCache',
    'moduleParsed',
    'beforeBuildEnd',
    'buildEnd',
  ])
  assert.deepEqual(globalThis.__arkdownHvigorBuildStart, [
    0,
    ['getFileName', 'getModuleIds', 'getModuleInfo', 'parse', 'share'],
  ])
  assert.deepEqual(globalThis.__arkdownHvigorTransform, [2, 'export {}', 'Index.ets'])
})

test('hvigorPluginFile and compilePluginPath independently reload the same module', async (context) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-hvigor-plugin-reload-'))
  const pluginFile = path.join(root, 'compile-plugin.cjs')
  fs.writeFileSync(pluginFile, "exports.default = { name: 'configured', transform(code) { return code } }\n")
  context.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const prepared = await new PluginAdapter(fakeBinding()).prepare({
    project: root,
    hvigorPluginFile: './compile-plugin.cjs',
    compilePluginPath: './compile-plugin.cjs',
  })
  assert.equal(prepared.plugins.length, 2)
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

test('ArkTS 1.1 accepts output-stage hooks while ESMODULE scan keeps them dormant', () => {
  let called = false
  const [plugin] = normalizePluginList(
    {
      name: 'chunk-plugin',
      renderChunk() {
        called = true
      },
      generateBundle() {
        called = true
      },
    },
    fakeBinding(),
  )
  assert.deepEqual(plugin.__arkdownPluginHooks, ['renderChunk', 'generateBundle'])
  assert.equal(called, false)
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

test('in-flight syscap registration resends can arrive out of order and keep real cached nodes', (t) => {
  const { invoke, request, calls, configs, ts } = sdkBatchFixture(t)
  const first = [request(1, true)]
  const second = [request(2, true)]
  assert.equal(invoke(second)[0].checkMessage, 'file2.ets')
  assert.equal(invoke(first)[0].checkMessage, 'file1.ets')
  assert.equal(invoke([request(1)])[0].checkMessage, 'file1.ets')
  assert.deepEqual(calls, ['file2.ets', 'file1.ets', 'file1.ets'])
  assert.ok(
    configs.every((config) => config === configs[0]),
    'resends preserve the plugin-visible config object identity',
  )
  assert.deepEqual(
    [...ts.creations],
    [
      ['file2.ets', 1],
      ['api.d.ets', 1],
      ['file1.ets', 1],
    ],
    'resending full immutable registrations does not reparse retained ASTs',
  )
})
