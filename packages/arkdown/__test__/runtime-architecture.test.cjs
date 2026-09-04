'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const test = require('node:test')

test('handwritten JavaScript is limited to entry, CLI, checker bridge, and plugin compatibility', () => {
  const packageRoot = path.resolve(__dirname, '..')
  const javascriptRoot = path.join(packageRoot, 'js')
  const sources = Object.fromEntries(
    fs
      .readdirSync(javascriptRoot)
      .filter((name) => /\.(?:c?js|mjs)$/.test(name))
      .sort()
      .map((name) => [name, fs.readFileSync(path.join(javascriptRoot, name), 'utf8')]),
  )
  const manifest = JSON.parse(fs.readFileSync(path.join(packageRoot, 'package.json'), 'utf8'))

  assert.deepEqual(Object.keys(sources), [
    'arkdown-cli.js',
    'ets-checker-bridge.cjs',
    'plugin-adapter.js',
    'runtime-entry.js',
  ])
  assert.equal(manifest.main, 'js/runtime-entry.js')
  assert.deepEqual(manifest.bin, { arkdown: 'js/arkdown-cli.js' })
  assert.equal(manifest.dependencies, undefined)
  assert.ok(Buffer.byteLength(sources['runtime-entry.js']) < 500)
  // The adapter owns the complete TS/JS plugin ABI, including V8 structured
  // clone for arbitrary plugin cache values. Build I/O and orchestration stay
  // native; keep the entire compatibility boundary below 34 KiB.
  assert.ok(Buffer.byteLength(sources['plugin-adapter.js']) < 34 * 1024)

  for (const removed of [
    'runtime.js',
    'hvigor-parameter-runtime.js',
    'hvigor-public-runtime.js',
    'hvigor-task-runtime.js',
    'hvigor-worker-runtime.js',
    'hvigor-worker.js',
  ]) {
    assert.equal(fs.existsSync(path.join(javascriptRoot, removed)), false, `${removed} must stay deleted`)
  }
})

test('plugin adapter exposes the complete supported lifecycle through one typed dispatcher', () => {
  const root = path.resolve(__dirname, '..', '..', '..')
  const adapter = fs.readFileSync(path.resolve(__dirname, '../js/plugin-adapter.js'), 'utf8')
  const napi = fs.readFileSync(path.resolve(__dirname, '../src/lib.rs'), 'utf8')
  const compiler = fs.readFileSync(path.join(root, 'crates/arkdown/src/plugin_module.rs'), 'utf8')

  for (const hook of [
    'beforeBuild',
    'buildStart',
    'afterBuildStart',
    'resolveId',
    'load',
    'transform',
    'moduleParsed',
    'resolveDynamicImport',
    'beforeBuildEnd',
    'buildEnd',
    'afterBuildEnd',
    'bundleEnd',
    'cleanUp',
    'shouldInvalidCache',
  ]) {
    assert.match(adapter, new RegExp(`'${hook}'`))
  }
  assert.match(adapter, /__arkdownDispatchPluginHook/)
  assert.match(adapter, /compilePluginModule/)
  assert.doesNotMatch(adapter, /typescript\.(?:transpile|transpileModule)/)
  assert.doesNotMatch(adapter, /hvigorfile/)
  assert.match(napi, /pub async fn compile_plugin_module/)
  assert.match(napi, /pub fn compile_plugin_module_sync/)
  assert.doesNotMatch(napi, /compile_hvigor_script/)
  assert.match(compiler, /pub struct PluginModuleCompiler/)
})

test('runtime entry performs no project, task, cache, or worker orchestration', () => {
  const entry = fs.readFileSync(path.resolve(__dirname, '../js/runtime-entry.js'), 'utf8')
  assert.doesNotMatch(entry, /(?:readFile|writeFile|readdir|Worker|Task|hvigorfile|runHvigorCommand)/)
  assert.match(entry, /createBuild/)
})

test('native package exposes direct commands and plugin bridges, not a Hvigor runtime', () => {
  const packageRoot = path.resolve(__dirname, '..')
  const sourceRoot = path.join(packageRoot, 'src')
  const napi = fs.readFileSync(path.join(sourceRoot, 'lib.rs'), 'utf8')
  const declarations = fs.readFileSync(path.join(packageRoot, 'index.d.ts'), 'utf8')

  for (const removed of [
    'build_script_runtime_model',
    'hvigor_runtime',
    'runtime_task',
    'hvigor_config_merge.rs',
    'hvigor_dependency_workspace.rs',
    'hvigor_maintenance.rs',
    'hvigor_sync.rs',
    'runtime_invocation.rs',
  ]) {
    assert.equal(fs.existsSync(path.join(sourceRoot, removed)), false, `${removed} must stay deleted`)
  }

  assert.doesNotMatch(napi, /run_runtime_tasks|RuntimeTaskBridgeSeed|RuntimeBuildScriptSession/)
  assert.doesNotMatch(declarations, /runRuntimeTasks|RuntimeHvigor|RuntimeBuildScriptSession/)
  assert.match(declarations, /build\(config: BuildConfig, plugins\?: Array<ArkdownPlugin>/)
  assert.match(declarations, /clean\(config: CleanConfig\): Promise<void>/)
  assert.match(declarations, /check\(config: CheckConfig\): Promise<void>/)
})

test('the npm JavaScript file is the only CLI entrypoint', () => {
  const packageRoot = path.resolve(__dirname, '..')
  const manifest = fs.readFileSync(path.join(packageRoot, 'Cargo.toml'), 'utf8')
  const workspaceManifest = fs.readFileSync(path.resolve(packageRoot, '../../Cargo.toml'), 'utf8')
  const napi = fs.readFileSync(path.join(packageRoot, 'src/lib.rs'), 'utf8')
  const declarations = fs.readFileSync(path.join(packageRoot, 'index.d.ts'), 'utf8')
  const cli = fs.readFileSync(path.join(packageRoot, 'js/arkdown-cli.js'), 'utf8')
  const observability = fs.readFileSync(path.join(packageRoot, 'src/runtime_observe/observability.rs'), 'utf8')

  assert.equal(fs.existsSync(path.join(packageRoot, 'src/main.rs')), false)
  assert.equal(fs.existsSync(path.join(packageRoot, 'src/runtime_observe/napi.rs')), false)
  assert.equal(fs.existsSync(path.join(packageRoot, 'src/runtime_observe/process_monitor.rs')), false)
  assert.doesNotMatch(manifest, /\[\[bin\]\]|clap\s*=/)
  assert.doesNotMatch(workspaceManifest, /^clap\s*=/m)
  assert.doesNotMatch(napi, /parse_arkdown_.*cli_args|clap::/)
  assert.doesNotMatch(observability, /install_cli|ObservabilityMode::Cli|FormatStyle::Cli/)
  assert.doesNotMatch(
    declarations,
    /parseArkdown.*CliArgs|installRuntimeObservability|RuntimeProcessMonitor|runtimeHvigorReportSnapshot/,
  )
  assert.match(cli, /class JsCliArgumentParser/)
})
