'use strict'

const assert = require('node:assert/strict')
const childProcess = require('node:child_process')
const fs = require('node:fs')
const path = require('node:path')
const test = require('node:test')

test('JavaScript remains plugin glue around Rust OXC Rolldown planners', () => {
  const runtime = fs.readFileSync(path.resolve(__dirname, '../runtime.js'), 'utf8')
  const cli = fs.readFileSync(path.resolve(__dirname, '../bin/arkdown.js'), 'utf8')
  const runtimeByteCount = Buffer.byteLength(runtime)
  assert.ok(
    runtimeByteCount <= 118000,
    `runtime.js grew to ${runtimeByteCount} bytes; the readable V8 boundary must remain bounded without minifying source`,
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

  const taskRoot = path.resolve(__dirname, '../src/runtime_task')
  const taskGraph = fs.readFileSync(path.join(taskRoot, 'graph.rs'), 'utf8')
  const taskModel = fs.readFileSync(path.join(taskRoot, 'model.rs'), 'utf8')
  const taskCommand = fs.readFileSync(path.join(taskRoot, 'command_plan.rs'), 'utf8')
  const taskEngine = fs.readFileSync(path.join(taskRoot, 'engine.rs'), 'utf8')
  const runtimeSession = fs.readFileSync(path.resolve(__dirname, '../src/build_script_runtime_model.rs'), 'utf8')
  assert.match(taskGraph, /pub struct RuntimeTaskGraph/)
  assert.match(taskModel, /pub struct RuntimeTaskModelBuilder/)
  assert.match(taskCommand, /pub\(crate\) struct RuntimeCommandPlanner/)
  assert.match(taskEngine, /pub struct RuntimeTaskEngine/)

  for (const removed of [
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
    assert.equal(runtime.includes(removed), false, `${removed} must not return to the JavaScript runtime`)
  }

  for (const nativeBoundary of [
    'compileHvigorScriptSync',
    'planHvigorCompilePlugins',
    'mergeHvigorConfig',
    'installRuntimeObservability',
    'emitRuntimeLog',
    'runtimeObservabilityIsEnabled',
    'finishRuntimeObservability',
    'persistRuntimeReport',
    'persistRuntimeDetails',
  ]) {
    assert.equal(runtime.includes(`nativeRuntimeBinding.${nativeBoundary}`), true)
  }

  assert.equal(runtime.includes('new nativeRuntimeBinding.RuntimeBuildScriptSession'), true)
  assert.equal(runtime.includes('._runtimeSession.beginInvocation'), true)
  assert.equal(runtime.includes('._runtimeSession.finishInvocation'), true)
  assert.equal(runtime.includes('._runtimeSession.captureHvigorOverrides'), true)
  assert.equal(runtime.includes('._runtimeSession.collectPluginPaths'), true)
  assert.equal(runtime.includes('._runtimeSession.resolvePluginPath'), true)
  assert.equal(runtime.includes('._runtimeSession.inspectRemoteHspObjects'), true)
  assert.equal(runtime.includes('._runtimeSession.runtimeTaskSelection'), true)
  assert.equal(runtime.includes('._runtimeSession.prepareNativeBuildConfig'), true)
  assert.equal(runtime.includes('._runtimeSession.prepareHvigorPluginWorkspace'), true)
  assert.equal(runtime.includes('._runtimeSession.prepareOhpmDependencyWorkspace'), true)
  assert.equal(runtime.includes('._runtimeSession.hvigorSyncVersionChanged'), true)
  assert.equal(runtime.includes('._runtimeSession.writeHvigorBuildMeta'), true)
  assert.equal(runtime.includes('._runtimeSession.writeHvigorSyncModel'), true)
  assert.equal(runtime.includes('._runtimeSession.runtimeCommandPlan'), true)
  assert.equal(runtime.includes('._runtimeSession.compilePluginRecords'), true)
  assert.equal(runtime.includes('._runtimeSession.executeCommandPlan'), true)
  assert.equal(runtime.includes('._runtimeSession.runtimeTaskCatalog'), true)
  assert.equal(runtime.includes('._runtimeSession.inspectRuntimeTaskModel'), true)
  assert.equal(runtime.includes('._runtimeSession.inspectRuntimeTaskState'), true)
  assert.equal(runtime.includes('._runtimeSession.inspectRuntimeTaskDeclarations'), true)
  assert.equal(runtime.includes('._runtimeSession.runStandaloneCommand'), true)
  assert.equal(runtime.includes('._runtimeSession.resolveCompilePluginPath'), true)
  assert.equal(runtimeSession.includes('pub async fn run_standalone_command'), true)
  assert.equal(runtimeSession.includes('pub fn resolve_compile_plugin_path'), true)
  assert.equal(runtimeSession.includes('runtime_task_model: Option<Value>'), true)
  assert.equal(runtime.includes('._runtimeSession.invalidateRuntimeTaskModel'), true)
  assert.equal(runtime.includes('_runtimeTaskInventoryCache'), false)
  assert.equal(taskModel.includes('fn qualify_overlay_dependencies'), true)
  assert.equal(taskCommand.includes('fn effective_execution_mode'), true)

  assert.match(runtime, /const official = load\([`'"]@ohos\/hvigor[`'"]\)/)

  assert.equal(runtime.includes("getLogger('debug-file')"), false)
  assert.match(runtime, /delete active\.appenders\[[`'"]debug-log-file[`'"]\]/)
  assert.match(runtime, /delete active\.categories\[[`'"]debug-file[`'"]\]/)
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

test('normal runtime startup does not load Hvigor-only logging and iconv packages', () => {
  const script = `
    const runtime = require(${JSON.stringify(path.resolve(__dirname, '../runtime.js'))})
    const host = new runtime.HvigorRuntimeHost(${JSON.stringify(
      path.resolve(__dirname, '../../../tests/fixtures/minimal-hap'),
    )})
    host.installObservability()
    const loaded = Object.keys(require.cache)
    process.stdout.write(JSON.stringify({
      iconv: loaded.some((file) => file.includes('/iconv-lite/')),
      log4js: loaded.some((file) => file.includes('/log4js/')),
    }))
    host.finishObservability()
  `
  const result = childProcess.spawnSync(process.execPath, ['-e', script], { encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(JSON.parse(result.stdout), { iconv: false, log4js: false })
})

test('Rust owns typed plugin callbacks and cache requests without JSON bridge text', () => {
  const runtime = fs.readFileSync(path.resolve(__dirname, '../runtime.js'), 'utf8')
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
