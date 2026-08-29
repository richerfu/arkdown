'use strict'

const assert = require('node:assert/strict')
const childProcess = require('node:child_process')
const fs = require('node:fs')
const path = require('node:path')
const test = require('node:test')

test('Hvigor compatibility keeps a bounded V8 boundary around Rust owners', () => {
  const packageRoot = path.resolve(__dirname, '..')
  const javascriptRoot = path.join(packageRoot, 'js')
  const runtime = fs.readFileSync(path.join(javascriptRoot, 'runtime.js'), 'utf8')
  const cli = fs.readFileSync(path.join(javascriptRoot, 'arkdown-cli.js'), 'utf8')
  const facadeNames = [
    'hvigor-public-runtime.js',
    'hvigor-parameter-runtime.js',
    'hvigor-task-runtime.js',
    'hvigor-worker-runtime.js',
    'hvigor-worker.js',
  ]
  const facades = facadeNames.map((name) => fs.readFileSync(path.join(javascriptRoot, name), 'utf8'))
  const manifest = JSON.parse(fs.readFileSync(path.join(packageRoot, 'package.json'), 'utf8'))
  const nativeRoot = path.join(packageRoot, 'src', 'hvigor_runtime')
  const native = fs
    .readdirSync(nativeRoot)
    .filter((name) => name.endsWith('.rs'))
    .map((name) => fs.readFileSync(path.join(nativeRoot, name), 'utf8'))
    .join('\n')
  const allRuntimeSources = [runtime, ...facades].join('\n')

  assert.deepEqual(
    fs
      .readdirSync(javascriptRoot)
      .filter((name) => /\.(?:c?js|mjs)$/.test(name))
      .sort(),
    [
      'arkdown-cli.js',
      'ets-checker-bridge.cjs',
      'hvigor-parameter-runtime.js',
      'hvigor-public-runtime.js',
      'hvigor-task-runtime.js',
      'hvigor-worker-runtime.js',
      'hvigor-worker.js',
      'runtime-entry.js',
      'runtime.js',
    ],
    'all handwritten production JavaScript must stay under js/',
  )
  assert.deepEqual(
    fs
      .readdirSync(packageRoot, { withFileTypes: true })
      .filter((entry) => entry.isFile() && /\.(?:c?js|mjs)$/.test(entry.name))
      .map((entry) => entry.name)
      .sort(),
    ['index.js', 'wasi-worker-browser.mjs'],
    'the package root may contain only generated N-API JavaScript access layers',
  )
  assert.equal(fs.existsSync(path.join(packageRoot, 'bin')), false)
  assert.equal(manifest.main, 'js/runtime-entry.js')
  assert.equal(manifest.bin.arkdown, 'js/arkdown-cli.js')
  assert.equal(manifest.bin.hvigorw, 'js/arkdown-cli.js')
  assert.equal(manifest.files.includes('js/'), true)

  assert.ok(Buffer.byteLength(runtime) <= 118000, 'runtime.js must remain bounded')
  assert.ok(
    facades.reduce((size, source) => size + Buffer.byteLength(source), 0) <= 40000,
    'all handwritten Hvigor facades, including the worker entry, must remain a thin boundary',
  )
  assert.ok(
    Buffer.byteLength(runtime) + facades.reduce((size, source) => size + Buffer.byteLength(source), 0) <= 145000,
    'splitting files must not evade the complete runtime JavaScript budget',
  )

  for (const privateHvigorOwner of [
    '@ohos/hvigor/src/',
    '_officialConfiguration',
    'ProjectImpl',
    'ModuleImpl',
    'HvigorNodeImpl',
    'HvigorLifecycleHook',
  ]) {
    assert.equal(runtime.includes(privateHvigorOwner), false, `${privateHvigorOwner} must remain Rust-owned glue`)
  }

  const taskRoot = path.join(packageRoot, 'src', 'runtime_task')
  const taskGraph = fs.readFileSync(path.join(taskRoot, 'graph.rs'), 'utf8')
  const taskModel = fs.readFileSync(path.join(taskRoot, 'model.rs'), 'utf8')
  const taskCommand = fs.readFileSync(path.join(taskRoot, 'command_plan.rs'), 'utf8')
  const taskEngine = fs.readFileSync(path.join(taskRoot, 'engine.rs'), 'utf8')
  const runtimeSession = fs.readFileSync(
    path.join(packageRoot, 'src', 'build_script_runtime_model', 'session.rs'),
    'utf8',
  )
  assert.match(taskGraph, /pub struct RuntimeTaskGraph/)
  assert.match(taskModel, /pub struct RuntimeTaskModelBuilder/)
  assert.match(taskCommand, /pub\(crate\) struct RuntimeCommandPlanner/)
  assert.match(taskEngine, /pub struct RuntimeTaskEngine/)

  for (const removedFromRuntime of [
    'typescript.transpileModule',
    "require('acorn')",
    'function collectCompilePluginPaths',
    'function resolvePluginPath',
    'function validateHvigorCompilePlugin',
    '_sdkRootFromEtsLoader',
    '_sdkComponentRoots',
    '_sdkComponentMetadata',
    '_remoteHspObject',
    '_remoteHspVersionWins',
    '_hasRollupPlugin',
    '_shouldBuildCmake',
    'loadBuildScriptRuntimeModel',
    'loadBuildScriptModuleModel',
    'planBuildScriptRuntime',
    '_runtimePlanCache',
    '_runtimePlanModules',
    '_runtimeModuleModelsByRoot',
    'inspectHvigorSdkDetails',
    'planHvigorRuntimeParameters',
    'nativeRuntimeBinding.discoverBuildScriptFiles',
    'nativeRuntimeBinding.planHvigorRuntimeProperties',
    'nativeRuntimeBinding.validateBuildScriptBuildOption',
    'nativeRuntimeBinding.prepareNativeBuildConfig',
    'nativeRuntimeBinding.prepareHvigorPluginWorkspace',
    'nativeRuntimeBinding.prepareOhpmDependencyWorkspace',
    'nativeRuntimeBinding.hvigorSyncVersionChanged',
    'nativeRuntimeBinding.writeHvigorBuildMeta',
    'nativeRuntimeBinding.writeHvigorSyncModel',
    'new nativeRuntimeBinding.RuntimeOhosContextSession',
    'new nativeRuntimeBinding.RuntimeTaskSelectionSession',
    '._runtimeSession.validateBuildOption',
    '._runtimeSession.runtimeContextOverrides',
    "Object.defineProperty(plugin, '__arkdownOrderIndex'",
    'delete plugin.order',
    'nativeRuntimeBinding.inspectRuntimeTaskInventory',
    'nativeRuntimeBinding.runtimeCommandPlan',
    'nativeRuntimeBinding.runHvigorClean',
    "'project_caches'",
    "'workspace', 'node_modules'",
  ]) {
    assert.equal(runtime.includes(removedFromRuntime), false, `${removedFromRuntime} must not return to runtime.js`)
  }

  for (const nativeBoundary of [
    'compileHvigorScriptSync',
    'planHvigorCompilePlugins',
    'mergeHvigorConfig',
    'installRuntimeObservability',
    'emitRuntimeLog',
    'finishRuntimeObservability',
    'persistRuntimeReport',
  ]) {
    assert.equal(allRuntimeSources.includes(`nativeRuntimeBinding.${nativeBoundary}`), true)
  }

  assert.equal(runtime.includes('new nativeRuntimeBinding.RuntimeBuildScriptSession'), true)
  for (const sessionMethod of [
    'beginInvocation',
    'finishInvocation',
    'captureHvigorOverrides',
    'collectPluginPaths',
    'resolvePluginPath',
    'inspectRemoteHspObjects',
    'runtimeTaskSelection',
    'prepareNativeBuildConfig',
    'prepareHvigorPluginWorkspace',
    'prepareOhpmDependencyWorkspace',
    'hvigorSyncVersionChanged',
    'writeHvigorBuildMeta',
    'writeHvigorSyncModel',
    'runtimeCommandPlan',
    'compilePluginRecords',
    'executeCommandPlan',
    'runtimeTaskCatalog',
    'inspectRuntimeTaskModel',
    'inspectRuntimeTaskState',
    'inspectRuntimeTaskDeclarations',
    'runStandaloneCommand',
    'resolveCompilePluginPath',
    'invalidateRuntimeTaskModel',
  ]) {
    assert.equal(runtime.includes(`._runtimeSession.${sessionMethod}`), true, sessionMethod)
  }
  assert.equal(runtimeSession.includes('pub async fn run_standalone_command'), true)
  assert.equal(runtimeSession.includes('pub fn resolve_compile_plugin_path'), true)
  assert.equal(runtimeSession.includes('runtime_task_model: Option<Value>'), true)
  assert.equal(runtime.includes('_runtimeTaskInventoryCache'), false)
  assert.equal(taskModel.includes('fn qualify_overlay_dependencies'), true)
  assert.equal(taskCommand.includes('fn effective_execution_mode'), true)

  for (const removed of [
    'hvigor-cache-runtime.js',
    'hvigor-constant-runtime.js',
    'hvigor-global-param-runtime.js',
    'hvigor-io-runtime.js',
    'hvigor-logging-runtime.js',
    'hvigor-observability-runtime.js',
    'hvigor-placeholder-runtime.js',
  ]) {
    assert.equal(fs.existsSync(path.join(javascriptRoot, removed)), false, `${removed} must stay deleted`)
    assert.equal(JSON.stringify(manifest.files).includes(removed), false, `${removed} must not be published`)
  }

  assert.doesNotMatch(runtime, /(?:require|load)\([`'"]@ohos\/hvigor[`'"]\)/)
  assert.equal(manifest.dependencies['@ohos/hvigor'], undefined)
  for (const owner of [
    'RuntimeHvigorFileApi',
    'RuntimeHvigorParameterState',
    'RuntimeHvigorTaskState',
    'RuntimeHvigorTaskHooks',
    'RuntimeHvigorWorkerPoolState',
  ]) {
    assert.match(native, new RegExp(`pub struct ${owner}`), `${owner} must own native behavior`)
  }
  for (const deletedOwner of [
    'RuntimeHvigorDirectedAcyclicGraph',
    'RuntimeHvigorGlobalParam',
    'RuntimeHvigorMetricStore',
    'RuntimeHvigorTraceState',
    'RuntimeHvigorDirectWorkerState',
  ]) {
    assert.doesNotMatch(native, new RegExp(`pub struct ${deletedOwner}`), `${deletedOwner} must stay deleted`)
  }

  const worker = facades[3]
  assert.match(worker, /new nativeRuntimeBinding\.RuntimeHvigorWorkerPoolState/)
  assert.doesNotMatch(worker, /class (?:NormalWorker|WatchWorker|WorkerPoolDelegator)/)
  assert.doesNotMatch(runtime, /class (?:MetricFactory|TraceManager|ReportServiceImpl|CacheStoreManager)/)
  assert.match(runtime, /runtimeHvigorReportSnapshot/)
  assert.match(runtime, /persistRuntimeReport/)

  assert.equal(runtime.includes("getLogger('debug-file')"), false)
  assert.equal(runtime.includes('writeStr(\n        this.projectTaskSnapShotCacheFilePath'), false)
  assert.equal(runtime.includes('return this.cacheTaskSnapShot.compareTo(this.curTaskSnapShot)'), false)
  assert.equal(runtime.includes('class HvigorSnapshotCacheService'), false)
  assert.equal(runtime.includes('class HvigorFileSnapshot {'), false)
  assert.equal(runtime.includes('class HvigorValueEntryView'), false)
  assert.equal(runtime.includes('class HvigorFileSnapshotView'), false)
  assert.equal(runtime.includes('class HvigorTaskSnapshotView'), false)
  assert.equal(runtime.includes('runtimeTaskCacheOperation'), false)
  assert.equal(runtime.includes('snapshotDeclarations'), false)
  assert.equal(runtime.includes('JSON.stringify(descriptor, hvigorReplacer)'), false)
  assert.doesNotMatch(runtime, /class\s+\w*(?:Task|File|Value)\w*Snapshot\b/)
  assert.equal(runtime.includes('compatibility stub'), false)
  assert.equal(runtime.includes("'task-cache.json'"), false)
  assert.equal(runtime.includes("'file-cache.json'"), false)
  assert.equal(runtime.includes('runtimeObservabilityInstalled'), false)
  assert.equal(runtime.includes('setInterval(() =>'), false)
  assert.equal(runtime.includes('process.memoryUsage()'), true)
  assert.equal(runtime.includes('ReportServiceImpl.data'), false)
  assert.equal(runtime.includes('persistRuntimeMonitor('), false)
  assert.equal(runtime.includes('new nativeRuntimeBinding.RuntimeProcessMonitor'), true)
  assert.equal(runtime.includes("fs.writeFileSync(path.resolve(directory, 'details.json')"), false)
  assert.equal(runtime.includes('fs.statSync(path.resolve(reportDirectory'), false)
  assert.equal(runtime.includes('function normalizeHookArguments'), false)
  assert.equal(runtime.includes('function normalizeHvigorCompileHookArguments'), false)
  assert.equal(runtime.includes('function splitRolldownHookArguments'), false)
  assert.equal(runtime.includes('_runtimeTaskPropertyRecords'), false)
  assert.equal(runtime.includes('_runtimeTaskFileDeclarations'), false)
  const startupImports = runtime.slice(0, runtime.indexOf('function createHvigorPublicUtilities'))
  assert.equal(startupImports.includes("require('iconv-lite')"), false)
  assert.equal(startupImports.includes("require('log4js')"), false)

  const pluginBridge = runtime.slice(
    runtime.indexOf('function normalizePluginHooks'),
    runtime.indexOf('function emitPluginPlanDiagnostics'),
  )
  assert.notEqual(pluginBridge.length, 0)
  assert.doesNotMatch(pluginBridge, /JSON\.(?:parse|stringify)/)
  assert.match(pluginBridge, /__arkdownDispatchPluginHook/)
  assert.doesNotMatch(pluginBridge, /normalized\.(?:buildStart|resolveId|load|transform|moduleParsed|buildEnd)\s*=/)

  assert.equal(runtime.includes('class ProjectCacheService'), false)
  assert.equal(runtime.includes('class TaskProxy'), false)
  assert.equal(runtime.includes("require('node:child_process')"), false)
  assert.equal(runtime.includes('installHvigorResolvePatch'), false)
  assert.equal(runtime.includes('hvigorDependencyContexts'), false)

  assert.equal(cli.includes('parseHvigorCliArgs(args)'), true)
  assert.equal(cli.includes('parseArkdownBuildCliArgs(args)'), true)
  assert.equal(cli.includes('const valueOptions = new Map'), false)
  assert.equal(cli.includes('switch (arg)'), false)
})

test('normal runtime startup does not load removed Hvigor implementation packages', () => {
  const script = `
    const Module = require('node:module')
    const original = Module._load
    const seen = new Set()
    Module._load = function(request, parent, isMain) {
      seen.add(request)
      return original.call(this, request, parent, isMain)
    }
    require(${JSON.stringify(path.resolve(__dirname, '../js/runtime.js'))})
    process.stdout.write(JSON.stringify({
      hvigor: seen.has('@ohos/hvigor'),
      iconv: seen.has('iconv-lite'),
      log4js: seen.has('log4js'),
    }))
  `
  const result = childProcess.spawnSync(process.execPath, ['-e', script], { encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(JSON.parse(result.stdout), { hvigor: false, iconv: false, log4js: false })
})

test('Rust owns typed plugin callbacks and cache requests without JSON bridge text', () => {
  const runtime = fs.readFileSync(path.resolve(__dirname, '../js/runtime.js'), 'utf8')
  const napi = fs.readFileSync(path.resolve(__dirname, '../src/lib.rs'), 'utf8')
  const callback = fs.readFileSync(path.resolve(__dirname, '../src/js_callback.rs'), 'utf8')
  const planner = fs.readFileSync(path.resolve(__dirname, '../src/hvigor_plugin_plan.rs'), 'utf8')
  const cache = fs.readFileSync(path.resolve(__dirname, '../../../crates/arkdown/src/cache_bridge.rs'), 'utf8')

  assert.match(napi, /MaybeAsyncJsCallback/)
  assert.match(napi, /type PluginDispatchTsfn/)
  assert.doesNotMatch(napi, /type (?:BuildStart|ResolveId|Load|Transform|ModuleParsed|BuildEnd)Tsfn/)
  assert.doesNotMatch(napi, /(?:build_start|resolve_id|load|transform|module_parsed|build_end)_fn:/)
  assert.match(napi, /HookUsage/)
  assert.equal(napi.includes('module_info_value'), false)
  assert.equal(planner.includes('plan_json'), false)
  assert.match(planner, /plan_value/)
  assert.equal(cache.includes('hook_request_json'), false)
  assert.match(cache, /hook_request_value/)
  assert.match(callback, /Promise<Ret>/)
  assert.doesNotMatch(
    runtime,
    /JSON\.parse\(\s*nativeRuntimeBinding\.(?:planHvigorCompilePlugins|planRolldownPackagePlugins|validateHvigorCompilePlugins|loadBuildScriptRuntimeModel|loadBuildScriptModuleModel|discoverBuildScriptFiles|inspectHvigorSdkDetails|planBuildScriptRuntime|planHvigorRuntimeParameters|planHvigorRuntimeProperties|validateBuildScriptBuildOption)/,
  )
  assert.doesNotMatch(
    runtime,
    /nativeRuntimeBinding\.(?:loadBuildScriptRuntimeModel|loadBuildScriptModuleModel|discoverBuildScriptFiles|inspectHvigorSdkDetails|planBuildScriptRuntime|planHvigorRuntimeParameters|planHvigorRuntimeProperties|validateBuildScriptBuildOption)\(\s*JSON\.stringify/,
  )
  assert.equal(runtime.includes('tasksJson'), false)
  assert.equal(runtime.includes('selectionJson'), false)
  assert.equal(runtime.includes('invokeNativeTaskOperation'), false)
  assert.match(napi, /dispatchTask/)
  assert.doesNotMatch(napi, /taskCallbacks/)
  assert.doesNotMatch(runtime, /_runtimeTaskCallbacks/)
  assert.match(callback, /StrongMaybeAsyncJsCallback/)
})
