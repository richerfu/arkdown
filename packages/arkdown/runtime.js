'use strict'

const fs = require('node:fs')
const crypto = require('node:crypto')
const Module = require('node:module')
const os = require('node:os')
const path = require('node:path')
const { pathToFileURL } = require('node:url')

// Hooks routed from JS runtime into Rolldown / BuildCoordinator.
// Lifecycle hooks not in this set (e.g. beforeBuild, cleanUp) are
// coordinator-side and are never forwarded to Rolldown's plugin pipeline.
const ROLLUP_HOOKS = new Set([
  'buildStart',
  'resolveId',
  'load',
  'transform',
  'moduleParsed',
  'beforeBuildEnd',
  'buildEnd',
  'shouldInvalidCache',
  'shouldTransformCachedModule',
  'resolveDynamicImport',
])

// All valid top-level attributes on an ArkdownPlugin / hvigor compile plugin.
// Unknown attributes produce a UNKNOWN_PLUGIN_ATTR warning, not a hard error.
const KNOWN_PLUGIN_ATTRS = new Set([
  'name',
  'order',
  // Rolldown / bundler hooks
  'buildStart',
  'resolveId',
  'load',
  'transform',
  'moduleParsed',
  'beforeBuildEnd',
  'buildEnd',
  'shouldInvalidCache',
  'shouldTransformCachedModule',
  'resolveDynamicImport',
  // Coordinator lifecycle hooks (not forwarded to Rolldown pipeline)
  'beforeBuild',
  'afterBuildStart',
  'afterBuildEnd',
  'cleanUp',
  // Context diagnostic handlers (called by Rust → JS to forward diagnostics)
  'warn',
  'error',
])

const hvigorDependencyContexts = []
let hvigorResolvePatchInstalled = false
let hvigorTypeScriptExtensionInstalled = false

function createBuild(nativeBuild) {
  return async function build(config = {}, plugins) {
    const runtime = new RuntimePluginLoader(config)
    const loadedPlugins = await runtime.load()
    const explicitPlugins = normalizePluginList(plugins, runtime.projectRoot)
    const allPlugins = [...loadedPlugins, ...explicitPlugins]
    return nativeBuild(toNativeBuildConfig(config), allPlugins)
  }
}

function toNativeBuildConfig(config) {
  const nativeConfig = { ...(config || {}) }
  delete nativeConfig.compilePluginPath
  delete nativeConfig.hvigorPluginFile
  return nativeConfig
}

class RuntimePluginLoader {
  constructor(config) {
    this.config = config || {}
    this.projectRoot = path.resolve(this.config.project || process.cwd())
    this.hvigorWorkspaceNodeModules = configureHvigorNodePath(this.projectRoot)
    this.seen = new Set()
  }

  async load() {
    const plugins = []
    for (const plugin of await this.loadConfiguredPluginPaths()) {
      plugins.push(plugin)
    }
    for (const plugin of await this.loadHvigorfiles()) {
      plugins.push(plugin)
    }
    return plugins
  }

  async loadConfiguredPluginPaths() {
    const plugins = []
    for (const key of ['hvigorPluginFile', 'compilePluginPath']) {
      for (const candidate of toArray(this.config[key])) {
        for (const plugin of await this.loadPluginPath(candidate, this.projectRoot, { requireDefault: true })) {
          plugins.push(plugin)
        }
      }
    }
    return plugins
  }

  async loadHvigorfiles() {
    const plugins = []
    for (const root of this.discoverHvigorRoots()) {
      for (const fileName of ['hvigorfile.js', 'hvigorfile.cjs', 'hvigorfile.mjs', 'hvigorfile.ts']) {
        const file = path.join(root, fileName)
        if (!fs.existsSync(file)) {
          continue
        }
        for (const plugin of await this.loadHvigorfile(file)) {
          plugins.push(plugin)
        }
      }
    }
    return plugins
  }

  discoverHvigorRoots() {
    const roots = [this.projectRoot]
    let entries = []
    try {
      entries = fs.readdirSync(this.projectRoot, { withFileTypes: true })
    } catch {
      return roots
    }

    for (const entry of entries) {
      if (!entry.isDirectory() || entry.name === 'node_modules' || entry.name.startsWith('.')) {
        continue
      }
      const candidate = path.join(this.projectRoot, entry.name)
      if (hasHvigorfile(candidate)) {
        roots.push(candidate)
      }
    }
    return roots
  }

  async loadHvigorfile(file) {
    // Create a host to capture `appTasks`/`hapTasks`/etc. config effects.
    const host = new HvigorRuntimeHost()
    const exports = await loadModule(file, host)
    const value = unwrapDefault(exports)
    const plugins = []
    const baseDir = path.dirname(file)

    for (const plugin of normalizePluginList(value, baseDir)) {
      plugins.push(plugin)
    }

    for (const plugin of normalizePluginList(value && value.plugins, baseDir)) {
      plugins.push(plugin)
    }

    // Collect plugin paths from the hvigorfile export itself.
    for (const candidate of collectHvigorPluginFiles(value)) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true })) {
        plugins.push(plugin)
      }
    }
    for (const candidate of collectCompilePluginPaths(value)) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true })) {
        plugins.push(plugin)
      }
    }

    // Collect plugin paths captured from task-factory calls (HvigorRuntimeHost effects).
    // This handles `hapTasks({ buildOption: { arkOptions: { compilePluginPath: '...' } } })`.
    for (const candidate of host._effects.hvigorPluginFile) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true })) {
        plugins.push(plugin)
      }
    }
    for (const candidate of host._effects.compilePluginPath) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true })) {
        plugins.push(plugin)
      }
    }

    return plugins
  }

  async loadPluginPath(candidate, baseDir = this.projectRoot, options = {}) {
    const file = resolvePluginPath(candidate, baseDir)
    if (!file || this.seen.has(file)) {
      return []
    }
    this.seen.add(file)
    const exports = await loadModule(file)
    const value = options.requireDefault ? requireDefaultExport(exports, file) : unwrapDefault(exports)
    return normalizeSortedPluginList(value, path.dirname(file))
  }
}

// ---------------------------------------------------------------------------
// HvigorRuntimeHost
// ---------------------------------------------------------------------------

/**
 * Simulates the hvigor task-factory host so that hvigorfile.ts/js code that
 * calls `appTasks({ buildOption: { arkOptions: { compilePluginPath } } })`
 * has its configuration effects captured by the runtime loader.
 *
 * hvigor's `hvigorfile.ts` calls `appTasks(opts)` / `hapTasks(opts)` where
 * `opts.buildOption.arkOptions.compilePluginPath` is the compile plugin path.
 * The host stubs these functions and records the resulting config so the
 * loader can discover plugins from those paths.
 *
 * # Upstream correspondence
 *
 * Mirrors hvigor's `TaskFactory.create(type, opts)` effect-capture path in
 * `hvigor-ohos-plugin`.
 */
class HvigorRuntimeHost {
  constructor() {
    this._effects = {
      /** Compile plugin paths collected from task factory calls. */
      compilePluginPath: [],
      /** hvigor plugin files collected from task factory calls. */
      hvigorPluginFile: [],
      target: null,
      buildOption: {},
    }
  }

  /**
   * Capture config effects from `appTasks`/`hapTasks`/`hspTasks`/`harTasks`.
   *
   * Accepts the same option shapes as hvigor's task factories:
   * - `opts.compilePluginPath` — direct compile plugin path
   * - `opts.hvigorPluginFile` — hvigor plugin file path
   * - `opts.buildOption.arkOptions.compilePluginPath` — nested under buildOption
   * - `opts.buildOption.arkOptions.hvigorPluginFile` — nested under buildOption
   */
  _captureTaskEffect(opts) {
    const o = opts || {}
    const ark = (o.buildOption && o.buildOption.arkOptions) || {}

    for (const cp of toArray(o.compilePluginPath).concat(toArray(ark.compilePluginPath))) {
      this._effects.compilePluginPath.push(cp)
    }
    for (const hp of toArray(o.hvigorPluginFile).concat(toArray(ark.hvigorPluginFile))) {
      this._effects.hvigorPluginFile.push(hp)
    }
    if (o.target) this._effects.target = o.target
    if (o.buildOption) Object.assign(this._effects.buildOption, o.buildOption)
    // Return a task-like object so `module.exports = { system: hapTasks(...) }` works.
    return { name: 'arkdown-stub' }
  }

  /**
   * Build the `@ohos/hvigor-ohos-plugin` mock object for this host.
   *
   * The task functions are bound to `this` so calling
   * `hapTasks({ buildOption: { ... } })` records the config.
   */
  _makeOhosPluginMock() {
    const factory = (opts) => this._captureTaskEffect(opts)
    return {
      appTasks: factory,
      hapTasks: factory,
      hspTasks: factory,
      harTasks: factory,
    }
  }
}

function collectCompilePluginPaths(value) {
  if (!value || typeof value !== 'object') {
    return []
  }
  const arkOptions = value.arkOptions || {}
  const buildArkOptions = (value.buildOption && value.buildOption.arkOptions) || {}
  const paths = [
    ...toArray(value.compilePluginPath),
    ...toArray(value.compilePluginFile),
    ...toArray(arkOptions.compilePluginPath),
    ...toArray(arkOptions.compilePluginFile),
    ...toArray(buildArkOptions.compilePluginPath),
    ...toArray(buildArkOptions.compilePluginFile),
  ]
  return paths
}

function collectHvigorPluginFiles(value) {
  if (!value || typeof value !== 'object') {
    return []
  }
  const arkOptions = value.arkOptions || {}
  const buildArkOptions = (value.buildOption && value.buildOption.arkOptions) || {}
  const paths = [
    ...toArray(value.hvigorPluginFile),
    ...toArray(arkOptions.hvigorPluginFile),
    ...toArray(buildArkOptions.hvigorPluginFile),
  ]
  return paths
}

// normalizePluginList does NOT apply order-as-function.  Used for inline plugins
// (hvigorfile body / explicit caller plugins) where order functions are not expected.
function normalizePluginList(value, baseDir) {
  const plugins = []
  appendNormalizedPlugins(plugins, unwrapDefault(value), baseDir)
  return plugins
}

// normalizeSortedPluginList is used for compilePluginPath / hvigorPluginFile
// modules that may carry an `order` function.  Collects plugins first, then
// applies function-order splicing so position is computed against the collected
// sibling set.
function normalizeSortedPluginList(value, baseDir) {
  const raw = []
  appendNormalizedPlugins(raw, unwrapDefault(value), baseDir)
  return applyFunctionOrder(raw)
}

// applyFunctionOrder resolves any plugin whose `order` is a function.
// Plugins with 'first'/'last' string order are kept as-is (Rust factory handles
// them relative to built-ins).  Function-order plugins are spliced into the
// current JS list at the index returned by the function.
function applyFunctionOrder(plugins) {
  const result = []
  const deferred = []

  // First pass: collect non-function-order plugins to build the name map
  for (const plugin of plugins) {
    if (typeof plugin.order === 'function') {
      deferred.push(plugin)
    } else {
      result.push(plugin)
    }
  }

  // Second pass: resolve function-order plugins
  for (const plugin of deferred) {
    const nameToIndex = new Map()
    result.forEach((p, i) => {
      // Deduplicate names by appending _N suffix, matching hvigor behaviour
      let name = p.name || ''
      let suffix = 1
      while (nameToIndex.has(name)) {
        name = `${p.name || ''}_${suffix}`
        suffix++
      }
      nameToIndex.set(name, i)
    })

    let idx
    try {
      idx = plugin.order(nameToIndex)
    } catch (err) {
      emitDiagnostic('error', {
        code: 'INVALID_PLUGIN_ORDER_FN',
        message: `Plugin "${plugin.name}": order function threw: ${err.message}`,
        pluginName: plugin.name,
      })
      // Fall back to appending at end
      result.push({ ...plugin, order: 'last' })
      continue
    }

    if (typeof idx !== 'number' || Number.isNaN(idx)) {
      emitDiagnostic('error', {
        code: 'INVALID_PLUGIN_ORDER_FN',
        message: `Plugin "${plugin.name}": order function must return a number, got ${JSON.stringify(idx)}`,
        pluginName: plugin.name,
      })
      result.push({ ...plugin, order: 'last' })
      continue
    }

    const clampedIdx = idx < 0 ? 0 : idx
    // Remove order from the normalized plugin so Rust does not try to parse
    // a function value — position is already resolved.
    const { order: _order, ...rest } = plugin
    result.splice(clampedIdx, 0, rest)
  }

  return result
}

function appendNormalizedPlugins(plugins, item, baseDir) {
  const value = unwrapDefault(item)
  if (!value) {
    return
  }
  if (typeof value === 'string') {
    return
  }
  if (typeof value === 'function') {
    appendNormalizedPlugins(plugins, value(), baseDir)
    return
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      appendNormalizedPlugins(plugins, item, baseDir)
    }
    return
  }
  if (typeof value === 'object' && hasRollupHook(value)) {
    validatePlugin(value)
    plugins.push(normalizeHookArguments(value))
    return
  }
  if (typeof value === 'object' && value.path) {
    return
  }
}

// validatePlugin enforces hvigor's plugin shape contract:
//   - name should be present (warning only)
//   - order must be 'first', 'last', a function, or absent
//   - unknown top-level attributes produce a warning
function validatePlugin(plugin) {
  if (!plugin.name) {
    emitDiagnostic('warn', {
      code: 'PLUGIN_NAME_NOT_EXIST',
      message: 'A compile plugin is missing a "name" field.',
    })
  }

  const order = plugin.order
  if (order !== undefined && order !== 'first' && order !== 'last' && typeof order !== 'function') {
    emitDiagnostic('error', {
      code: 'PLUGIN_NAME_WITH_UNKNOWN_ORDER',
      message: `Plugin "${plugin.name}": unknown order value ${JSON.stringify(order)}. Expected 'first', 'last', or a function.`,
      pluginName: plugin.name,
    })
  }

  for (const key of Object.keys(plugin)) {
    if (!KNOWN_PLUGIN_ATTRS.has(key)) {
      emitDiagnostic('warn', {
        code: 'UNKNOWN_PLUGIN_ATTR',
        message: `Plugin "${plugin.name}": unknown attribute "${key}" will be ignored.`,
        pluginName: plugin.name,
      })
    }
  }
}

// emitDiagnostic is the central place to emit structured diagnostics.
// Errors throw; warnings go through the host warning channel.
function emitDiagnostic(level, diagnostic) {
  if (level === 'error') {
    const err = new Error(diagnostic.message)
    err.code = diagnostic.code
    if (diagnostic.pluginName) err.pluginName = diagnostic.pluginName
    throw err
  }
  // warning
  emitWarningDiagnostic(diagnostic)
}

function emitWarningDiagnostic(diagnostic) {
  const message = `[arkdown] ${diagnostic.code}: ${diagnostic.message}`
  if (typeof process !== 'undefined' && typeof process.emitWarning === 'function') {
    process.emitWarning(message, {
      code: diagnostic.code,
      type: 'ArkdownWarning',
    })
    return
  }
  if (typeof globalThis.dispatchEvent === 'function' && typeof globalThis.CustomEvent === 'function') {
    globalThis.dispatchEvent(new CustomEvent('arkdown:warning', { detail: diagnostic }))
  }
}

// ---------------------------------------------------------------------------
// Plugin context binding — hvigor's doWrapper / getCompilePluginContext
// ---------------------------------------------------------------------------

/**
 * Per-build share context captured when `beforeBuild` fires.
 *
 * hvigor's `getCompilePluginContext(this).share` is a `projectConfig` object
 * containing bundleName, useNormalizedOHMUrl, buildMode, etc.  We store it
 * here when the coordinator calls `beforeBuild(shareJson)` so that it is
 * available to all subsequent hook calls in the same build.
 *
 * Thread safety: Node.js event loop is single-threaded; NAPI calls into JS
 * from Rust TSFNs are serialised through the event loop, so no locking is
 * needed.
 */
let _pluginShareContext = {}

/**
 * Parse the JSON context string passed from Rust to a lifecycle hook.
 * Returns an empty object if the string is absent/invalid/not-JSON.
 */
function parsePluginContext(ctxJson) {
  if (!ctxJson || typeof ctxJson !== 'string' || ctxJson === '') return {}
  try {
    const parsed = JSON.parse(ctxJson)
    return typeof parsed === 'object' && parsed !== null ? parsed : {}
  } catch {
    return {}
  }
}

/**
 * Build a full hvigor-compatible compile plugin context object.
 *
 * Merges the share context captured from `beforeBuild` with any additional
 * fields from the current hook's context JSON.
 *
 * Shape: `{ share: { bundleName, buildMode, … }, getModuleIds(), getModuleInfo(), … }`
 *
 * # Upstream correspondence
 *
 * Corresponds to hvigor's `getCompilePluginContext(this)` result from
 * `arkts-pack.ts`.  User plugins access `this.share.projectConfig.bundleName`,
 * `this.getModuleIds()`, etc., through this object.
 */
function buildPluginContext(hookCtx) {
  const share = hookCtx.share || _pluginShareContext
  const moduleIds = hookCtx.moduleIds || []
  const moduleInfoMap = hookCtx.moduleInfoMap || {}
  return {
    // hvigor share context
    share,
    // getModuleIds() — returns collected module IDs
    getModuleIds() {
      return moduleIds
    },
    // getModuleInfo(id) — returns { importedIds, dynamicallyImportedIds }
    getModuleInfo(id) {
      return moduleInfoMap[id] || null
    },
    // parse() — stub; use OXC directly for actual parsing
    parse(_source, _options) {
      return null
    },
    // getFileName() — not yet implemented
    getFileName(_ref) {
      return null
    },
  }
}

function normalizeHookArguments(plugin) {
  const patches = {}

  // ── Lifecycle hooks: bind context as `this` ──────────────────────────────
  // hvigor's `doWrapper` / `getCompilePluginContext` pattern: each hook is
  // called with `this` = compile plugin context.  We replicate this by
  // parsing the JSON context argument and binding it as `this`.
  //
  // Hooks that receive a JSON context string from Rust:
  //   beforeBuild(shareJson)     — coordinator calls with share context
  //   afterBuildStart(shareJson) — coordinator calls with share context
  //   buildStart(ctxJson)        — Rolldown hook, currently empty string
  //   beforeBuildEnd(ctxJson)    — { moduleIds }
  //   buildEnd(ctxJson)          — { errorCount, moduleInfoMap }
  //   afterBuildEnd(ctxJson)     — { success }
  //   cleanUp(ctxJson)           — empty string

  const ctxHooks = ['beforeBuild', 'afterBuildStart', 'buildStart', 'beforeBuildEnd', 'buildEnd', 'afterBuildEnd', 'cleanUp']
  for (const hookName of ctxHooks) {
    if (typeof plugin[hookName] === 'function') {
      const original = plugin[hookName]
      patches[hookName] = function(ctxJson) {
        const hookCtx = parsePluginContext(ctxJson)
        // Capture share from beforeBuild for use in subsequent hooks.
        if (hookName === 'beforeBuild' && hookCtx.share) {
          _pluginShareContext = hookCtx.share
        }
        const ctx = buildPluginContext(hookCtx)
        return original.call(ctx, ctxJson)
      }
    }
  }

  // ── Cache/transform hooks: parse request JSON ────────────────────────────

  if (typeof plugin.shouldInvalidCache === 'function') {
    const original = plugin.shouldInvalidCache
    patches.shouldInvalidCache = function shouldInvalidCache(request) {
      return original.call(this, parseShouldInvalidCacheRequest(request))
    }
  }

  if (typeof plugin.shouldTransformCachedModule === 'function') {
    const original = plugin.shouldTransformCachedModule
    patches.shouldTransformCachedModule = function shouldTransformCachedModule(request) {
      return original.call(this, parseShouldInvalidCacheRequest(request))
    }
  }

  if (Object.keys(patches).length === 0) {
    return plugin
  }
  return { ...plugin, ...patches }
}

function parseShouldInvalidCacheRequest(request) {
  if (typeof request !== 'string') {
    return request
  }
  try {
    return JSON.parse(request)
  } catch {
    return { cacheHit: Boolean(request) }
  }
}

// All hook names that make a plugin object "active" and eligible for
// inclusion in the normalized plugin list.  This includes both Rolldown
// bundler hooks AND coordinator lifecycle hooks.
const ALL_PLUGIN_HOOKS = new Set([
  ...ROLLUP_HOOKS,
  // Coordinator lifecycle hooks
  'beforeBuild',
  'afterBuildStart',
  'afterBuildEnd',
  'cleanUp',
  // Diagnostic context handlers
  'warn',
  'error',
])

function hasRollupHook(plugin) {
  return Array.from(ALL_PLUGIN_HOOKS).some((hook) => typeof plugin[hook] === 'function')
}

function hasHvigorfile(root) {
  return ['hvigorfile.js', 'hvigorfile.cjs', 'hvigorfile.mjs', 'hvigorfile.ts'].some((fileName) =>
    fs.existsSync(path.join(root, fileName)),
  )
}

function configureHvigorNodePath(projectRoot) {
  const hvigorConfig = path.join(projectRoot, 'hvigor', 'hvigor-config.json5')
  if (!fs.existsSync(hvigorConfig)) {
    return null
  }
  const config = readProjectHvigorConfig(hvigorConfig)
  const workspace = path.join(getHvigorUserHome(), 'project_caches', hvigorProjectHash(projectRoot), 'workspace')
  const nodeModules = path.join(workspace, 'node_modules')
  process.env.WORKSPACE_DIR = workspace
  process.env.NODE_PATH = process.env.NODE_PATH ? `${nodeModules}${path.delimiter}${process.env.NODE_PATH}` : nodeModules
  Module._initPaths()
  installHvigorTypeScriptExtension()
  installHvigorResolvePatch()
  hvigorDependencyContexts.unshift({ nodeModules, dependencies: config.dependencies })
  return nodeModules
}

function getHvigorUserHome() {
  if (process.env.HVIGOR_USER_HOME && path.isAbsolute(process.env.HVIGOR_USER_HOME)) {
    return process.env.HVIGOR_USER_HOME
  }
  return path.resolve(os.homedir(), '.hvigor')
}

function hvigorProjectHash(projectRoot) {
  return crypto.createHash('sha256').update(path.resolve(projectRoot)).digest('hex')
}

function readProjectHvigorConfig(file) {
  const text = fs.readFileSync(file, 'utf8')
  const config = Function(`"use strict"; return (${text}\n);`)()
  const dependencies = config && config.dependencies && typeof config.dependencies === 'object' ? { ...config.dependencies } : {}
  delete dependencies['@ohos/hvigor']
  delete dependencies['@ohos/hvigor-ohos-plugin']
  return { ...config, dependencies }
}

function installHvigorTypeScriptExtension() {
  if (hvigorTypeScriptExtensionInstalled) {
    return
  }
  Module._extensions['.ts'] = function hvigorTypeScriptExtension(mod, filename) {
    const source = fs.readFileSync(filename, 'utf8')
    const ts = require('typescript')
    const output = ts.transpileModule(source, {
      compilerOptions: {
        target: ts.ScriptTarget.ES2016,
        module: ts.ModuleKind.CommonJS,
        types: [],
        allowJs: true,
        esModuleInterop: true,
        forceConsistentCasingInFileNames: true,
        strict: true,
        skipLibCheck: true,
        noImplicitAny: true,
        noImplicitReturns: true,
        alwaysStrict: true,
        declaration: true,
        noEmit: true,
      },
      fileName: filename,
    }).outputText
    mod._compile(output, filename)
  }
  hvigorTypeScriptExtensionInstalled = true
}

function installHvigorResolvePatch() {
  if (hvigorResolvePatchInstalled) {
    return
  }
  const originalResolveFilename = Module._resolveFilename
  Module._resolveFilename = function hvigorResolveFilename(request, parent, isMain, options) {
    try {
      return originalResolveFilename.call(this, request, parent, isMain, options)
    } catch (err) {
      const resolved = resolveHvigorDependencyIndex(request)
      if (resolved) {
        return resolved
      }
      throw err
    }
  }
  hvigorResolvePatchInstalled = true
}

function resolveHvigorDependencyIndex(request) {
  for (const context of hvigorDependencyContexts) {
    if (!Object.prototype.hasOwnProperty.call(context.dependencies, request)) {
      continue
    }
    const packageRoot = path.resolve(context.nodeModules, request)
    for (const ext of ['.js', '.ts']) {
      const indexFile = path.join(packageRoot, `index${ext}`)
      if (fs.existsSync(indexFile)) {
        return indexFile
      }
    }
  }
  return null
}

// host is an optional HvigorRuntimeHost — when provided, @ohos/hvigor-ohos-plugin
// returns callable task factories that capture config effects.
async function loadModule(file, host) {
  const ext = path.extname(file)
  if (ext === '.mjs') {
    return import(pathToFileURL(file).href)
  }
  if (ext === '.ts') {
    return loadTypeScriptModule(file, host)
  }
  // Clear the require cache so that reloads (watch mode, test isolation)
  // always get a fresh copy — matching hvigor's behaviour.
  const resolved = require.resolve(file)
  delete require.cache[resolved]
  return require(resolved)
}

// host: optional HvigorRuntimeHost for capturing task-factory config effects.
function loadTypeScriptModule(file, host) {
  const source = fs.readFileSync(file, 'utf8')
  let code = source
  try {
    const ts = require('typescript')
    code = ts.transpileModule(source, {
      compilerOptions: {
        module: ts.ModuleKind.CommonJS,
        target: ts.ScriptTarget.ES2020,
        esModuleInterop: true,
      },
      fileName: file,
    }).outputText
  } catch {
    throw new Error(`loading ${file} requires the 'typescript' package`)
  }

  const mod = new Module(file, module.parent || module)
  mod.filename = file
  mod.paths = Module._nodeModulePaths(path.dirname(file)).concat(Module.globalPaths)
  const originalRequire = mod.require.bind(mod)
  mod.require = (id) => {
    if (id === '@ohos/hvigor-ohos-plugin') {
      if (host) {
        return host._makeOhosPluginMock()
      }
    }
    return originalRequire(id)
  }
  mod._compile(code, file)
  return mod.exports
}

function unwrapDefault(value) {
  if (value && typeof value === 'object' && 'default' in value) {
    return value.default
  }
  return value
}

function requireDefaultExport(value, file) {
  if (value && typeof value === 'object' && 'default' in value && value.default) {
    return value.default
  }
  const err = new Error(`PLUGIN_FILE_NO_DEFAULT_EXPORT: ${file}`)
  err.code = 'PLUGIN_FILE_NO_DEFAULT_EXPORT'
  throw err
}

function resolvePluginPath(candidate, baseDir) {
  if (!candidate || typeof candidate !== 'string') {
    return null
  }
  const resolved = path.isAbsolute(candidate) ? candidate : path.resolve(baseDir, candidate)
  if (fs.existsSync(resolved)) {
    return resolved
  }
  for (const ext of ['.js', '.cjs', '.mjs', '.ts']) {
    if (fs.existsSync(`${resolved}${ext}`)) {
      return `${resolved}${ext}`
    }
  }
  return resolved
}

function toArray(value) {
  if (value == null) {
    return []
  }
  return Array.isArray(value) ? value : [value]
}

module.exports = {
  HvigorRuntimeHost,
  RuntimePluginLoader,
  createBuild,
  toNativeBuildConfig,
  hasHvigorfile,
  normalizePluginList,
  normalizeSortedPluginList,
  applyFunctionOrder,
  parseShouldInvalidCacheRequest,
  parsePluginContext,
  buildPluginContext,
  requireDefaultExport,
  configureHvigorNodePath,
  hvigorProjectHash,
  readProjectHvigorConfig,
}
