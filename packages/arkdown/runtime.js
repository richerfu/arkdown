'use strict'

const fs = require('node:fs')
const crypto = require('node:crypto')
const Module = require('node:module')
const os = require('node:os')
const path = require('node:path')
const acorn = require('acorn')

// Behavior baseline verified against the hvigor bundled with DevEco Studio
// 6.1.1. Keep this explicit so compatibility changes are evidence-driven.
const HVIGOR_PARITY_VERSION = '6.24.3'

// Hooks routed into Rolldown. The two latest-hvigor additions that Rolldown
// does not own are handled by a thin internal adapter below this interface.
const ROLLUP_HOOKS = new Set([
  'buildStart',
  'resolveId',
  'load',
  'transform',
  'moduleParsed',
  'buildEnd',
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
  'buildEnd',
  'resolveDynamicImport',
  // Latest-hvigor gaps supplied around Rolldown.
  'beforeBuildEnd',
  'shouldInvalidCache',
])

// Exact public compile-plugin surface in the latest IDE's
// @ohos/hvigor-arkts-compose/dist/src/hvigor-plugins/types.d.ts.
const HVIGOR_COMPILE_HOOKS = new Set([
  'buildStart',
  'resolveId',
  'load',
  'transform',
  'shouldInvalidCache',
  'moduleParsed',
  'beforeBuildEnd',
  'buildEnd',
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
    await runtime.hvigorHost.runTaskGraphResolved()
    try {
      // Hvigor tasks belong to the build task graph, not the bundler plugin
      // driver. Package-bound custom tasks therefore run at the native build
      // boundary instead of being disguised as a non-Rolldown plugin hook.
      await runtime.hvigorHost.runPackageBoundCustomTasks()
      const result = await nativeBuild(runtime.toNativeBuildConfig(), allPlugins)
      await runtime.hvigorHost.runBuildFinished(null, result)
      return result
    } catch (error) {
      await runtime.hvigorHost.runBuildFinished(error)
      throw error
    }
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
    this.hvigorHost = new HvigorRuntimeHost(this.projectRoot, this.config)
    this.hvigorHost.hvigorWorkspaceNodeModules = this.hvigorWorkspaceNodeModules
    this.seen = new Set()
    this.projectHvigorOverrides = {}
    this.moduleHvigorOverrides = new Map()
  }

  async load() {
    const plugins = []
    await this.hvigorHost.evaluateHvigorConfig()
    for (const plugin of await this.loadConfiguredPluginPaths()) {
      plugins.push(plugin)
    }
    for (const plugin of await this.loadHvigorfiles()) {
      plugins.push(plugin)
    }
    for (const plugin of await this.loadBuildProfileCompilePlugins()) {
      plugins.push(plugin)
    }
    return new HvigorCompilePluginPlanner(this.config).apply(plugins)
  }

  toNativeBuildConfig() {
    const nativeConfig = toNativeBuildConfig(this.config)
    // These fields are runtime-owned. Do not accept caller-provided serialized
    // values that could bypass hvigorfile evaluation and scope discovery.
    delete nativeConfig.projectHvigorOverridesJson
    delete nativeConfig.moduleHvigorOverridesJson
    if (Object.keys(this.projectHvigorOverrides).length > 0) {
      nativeConfig.projectHvigorOverridesJson = this.serializeOverrides(
        this.projectHvigorOverrides,
        'project hvigor overrides',
      )
    }
    if (this.moduleHvigorOverrides.size > 0) {
      nativeConfig.moduleHvigorOverridesJson = this.serializeOverrides(
        Object.fromEntries(this.moduleHvigorOverrides),
        'module hvigor overrides',
      )
    }
    return nativeConfig
  }

  serializeOverrides(value, label) {
    try {
      return JSON.stringify(value)
    } catch (error) {
      throw new TypeError(`failed to serialize ${label}: ${error.message}`)
    }
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
    const records = []
    const files = []
    for (const root of this.discoverHvigorRoots()) {
      // @ohos/hvigor 6.24.3 HvigorCommonConst.BUILD_FILE_NAME_SUFFIX:
      // select exactly one file in .ts, .js, .cjs, .mjs precedence order.
      for (const fileName of ['hvigorfile.ts', 'hvigorfile.js', 'hvigorfile.cjs', 'hvigorfile.mjs']) {
        const file = path.join(root, fileName)
        if (fs.existsSync(file)) {
          files.push(file)
          break
        }
      }
    }
    this.hvigorHost.prepareHvigorfiles(files)
    await this.hvigorHost.runNodesInitialized()
    for (const file of files) {
      records.push(await this.loadHvigorfile(file))
    }
    await this.hvigorHost.runNodesEvaluated()
    for (const record of records) {
      for (const plugin of await this.collectHvigorfileEffects(record)) {
        plugins.push(plugin)
      }
    }
    return plugins
  }

  discoverHvigorRoots() {
    const roots = [this.projectRoot]
    const declaredModuleRoots = this.hvigorHost.moduleRoots()
    if (declaredModuleRoots.length > 0) {
      roots.push(...declaredModuleRoots)
      return roots
    }
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
    // Hvigor owns one node/lifecycle host for the complete configuration
    // phase. Project and module hvigorfiles must therefore share the host.
    const host = this.hvigorHost
    await host.beginHvigorfile(file)
    const exports = await loadModule(file, host)
    const value = unwrapDefault(exports)
    host.bindHvigorfile(file, value)
    await host.applyCustomPlugins(file, value && value.plugins)
    await host.runAfterNodeEvaluate(file)
    return { file, value }
  }

  async collectHvigorfileEffects({ file, value }) {
    const host = this.hvigorHost
    const plugins = []
    const baseDir = path.dirname(file)
    const moduleRoot = path.resolve(baseDir) === this.projectRoot ? undefined : path.resolve(baseDir)
    const effects = host.effectsFor(file)
    const capturedBuildOption = this.captureHvigorOverrides(baseDir, value, effects, file)

    // A hvigorfile `plugins` array contains HvigorPlugin objects with
    // `pluginId/apply`, not Rolldown hooks. They were applied above after the
    // system plugin was bound, matching hvigor 6.24.3 configuration().
    for (const plugin of normalizePluginList(value, baseDir)) {
      plugins.push(plugin)
    }

    // Collect plugin paths from the hvigorfile export itself.
    for (const candidate of collectHvigorPluginFiles(value)) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true, moduleRoot })) {
        plugins.push(plugin)
      }
    }
    for (const candidate of collectCompilePluginPaths(value)) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true, moduleRoot })) {
        plugins.push(plugin)
      }
    }
    // Programmatic task-factory and module-context mutations are collected
    // from the same merged object that is forwarded to Rust. The latest IDE
    // schemas reject a declarative `compilePluginFile`; that case is diagnosed
    // before this compatibility path is reached.
    for (const candidate of collectCompilePluginPaths(capturedBuildOption, { includeCompilePluginFile: false })) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true, moduleRoot })) {
        plugins.push(plugin)
      }
    }

    // Collect plugin paths captured from task-factory calls (HvigorRuntimeHost effects).
    // This handles `hapTasks({ buildOption: { arkOptions: { compilePluginPath: '...' } } })`.
    for (const candidate of effects.hvigorPluginFile) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true, moduleRoot })) {
        plugins.push(plugin)
      }
    }
    for (const candidate of effects.compilePluginPath) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true, moduleRoot })) {
        plugins.push(plugin)
      }
    }

    return plugins
  }

  async loadBuildProfileCompilePlugins() {
    const plugins = []
    for (const record of this.hvigorHost.compilePluginRecords(this.moduleHvigorOverrides)) {
      for (const plugin of await this.loadPluginPath(record.file, record.moduleRoot, {
        requireDefault: true,
        moduleRoot: record.moduleRoot,
        targetName: record.targetName,
      })) {
        plugins.push(plugin)
      }
    }
    return plugins
  }

  captureHvigorOverrides(root, value, effects, file = path.join(root, 'hvigorfile.ts')) {
    const ohos = value && typeof value === 'object' ? value.config?.ohos : undefined
    const overrides = ohos && typeof ohos === 'object' ? ohos.overrides : undefined
    const normalizedRoot = path.resolve(root)

    if (normalizedRoot === this.projectRoot) {
      if (overrides && typeof overrides === 'object') {
        if (overrides.appOpt !== undefined) {
          this.projectHvigorOverrides.appOpt = mergeHvigorConfig(this.projectHvigorOverrides.appOpt, overrides.appOpt)
        }
        if (overrides.signingConfig !== undefined) {
          this.projectHvigorOverrides.signingConfig = mergeHvigorConfig(
            this.projectHvigorOverrides.signingConfig,
            overrides.signingConfig,
          )
        }
      }
      return undefined
    }

    const exportedBuildOption = overrides && typeof overrides === 'object' ? overrides.buildOption : undefined
    this.hvigorHost.assertNoDeclarativeCompilePluginFile(
      exportedBuildOption,
      file,
      'config.ohos.overrides.buildOption',
    )
    // `effects.buildOption` retains compatibility with task-factory calls
    // used by older projects. Declarative `config.ohos.overrides.buildOption`
    // is applied last, matching hvigor's module config manager.
    const capturedBuildOption = effects.buildOption
    const buildOption = mergeHvigorConfig(
      capturedBuildOption && Object.keys(capturedBuildOption).length > 0 ? capturedBuildOption : undefined,
      exportedBuildOption,
    )
    if (buildOption !== undefined) {
      this.moduleHvigorOverrides.set(
        normalizedRoot,
        mergeHvigorConfig(this.moduleHvigorOverrides.get(normalizedRoot), buildOption),
      )
    }
    return buildOption
  }

  async loadPluginPath(candidate, baseDir = this.projectRoot, options = {}) {
    const file = resolvePluginPath(candidate, baseDir)
    const moduleRoot = options.moduleRoot ? path.resolve(options.moduleRoot) : undefined
    const targetName = typeof options.targetName === 'string' ? options.targetName : undefined
    const identity = `${file || ''}\0${moduleRoot || '*'}\0${targetName || '*'}`
    if (!file || this.seen.has(identity)) {
      return []
    }
    this.seen.add(identity)
    const exports = await loadModule(file)
    const value = options.requireDefault ? requireDefaultExport(exports, file) : unwrapDefault(exports)
    return normalizeSortedPluginList(value, path.dirname(file), file, moduleRoot, targetName)
  }
}

// ---------------------------------------------------------------------------
// HvigorRuntimeHost
// ---------------------------------------------------------------------------

class HvigorNormalizedFile {
  constructor(filePath) {
    this.filePath = path.resolve(filePath)
    Object.freeze(this)
  }

  asFileList() {
    if (!fs.existsSync(this.filePath)) {
      return []
    }
    const files = [this]
    if (!fs.statSync(this.filePath).isDirectory()) {
      return files
    }
    const pending = [this.filePath]
    while (pending.length > 0) {
      const directory = pending.pop()
      const entries = fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))
      for (const entry of entries) {
        const candidate = path.join(directory, entry.name)
        files.push(new HvigorNormalizedFile(candidate))
        if (entry.isDirectory()) {
          pending.push(candidate)
        }
      }
    }
    return files
  }

  file(relativePath) {
    const candidate = path.resolve(this.filePath, relativePath)
    return fs.existsSync(candidate) ? new HvigorNormalizedFile(candidate) : undefined
  }

  getPath() {
    return this.filePath
  }
}

class HvigorTaskInput {
  constructor() {
    this.declarations = { files: [], properties: new Map() }
  }

  file(filePath) {
    this.declarations.files.push(filePath)
    return this
  }

  files(filePaths) {
    this.declarations.files.push(...filePaths)
    return this
  }

  property(key, value) {
    this.declarations.properties.set(key, value)
    return this
  }
}

class HvigorTaskOutput {
  constructor() {
    this.declarations = { files: [] }
  }

  file(filePath) {
    this.declarations.files.push(filePath)
    return this
  }

  files(filePaths) {
    this.declarations.files.push(...filePaths)
    return this
  }
}

/**
 * Public Task wrapper used by HvigorNodeImpl in @ohos/hvigor 6.24.3.
 *
 * CoreTaskImpl stores before hooks in a stack and after hooks in a queue. Keep
 * those data structures explicit here so hooks added through getTaskByName()
 * compose with hooks declared on HvigorTask in exactly the same direction.
 */
class HvigorTaskView {
  constructor(task) {
    this.task = task
  }

  getName() {
    return this.task.definition.name
  }

  getDependencies() {
    return Array.isArray(this.task.definition.dependencies) ? [...this.task.definition.dependencies] : []
  }

  setEnable(enabled) {
    this.task.enabled = enabled
  }

  beforeRun(callback) {
    this.task.beforeRunHooks.push({ callback, declared: false })
  }

  afterRun(callback) {
    this.task.afterRunHooks.push({ callback, declared: false })
  }
}

class HvigorNodeDescriptorView {
  constructor(name, srcPath, rootDescriptor, extraOptions = {}) {
    this.name = name
    this.srcPath = srcPath
    this.extraOptions = new Map(Object.entries(extraOptions || {}))
    this.rootDescriptor = rootDescriptor || this
    this.children = undefined
  }

  getChildNode() {
    return this.children
  }

  getRootNode() {
    return this.rootDescriptor
  }
}

/**
 * Mutable pre-node model exposed to hvigorconfig.ts.
 *
 * Mirrors latest IDE HvigorConfig: include/exclude updates descriptors before
 * Hvigor nodes are initialized; later lifecycle callbacks observe the updated
 * node tree.
 */
class HvigorConfigView {
  constructor(host) {
    this.host = host
    this.rootDescriptor = new HvigorNodeDescriptorView(path.basename(host.projectRoot), host.projectRoot)
    this.children = host._moduleDescriptors.map((descriptor) =>
      this._child(descriptor.name, descriptor.srcPath || path.relative(host.projectRoot, descriptor.root), descriptor),
    )
    this.rootDescriptor.children = this.children
  }

  _child(name, srcPath, descriptor = {}) {
    const child = new HvigorNodeDescriptorView(name, srcPath, this.rootDescriptor, descriptor.extraOptions)
    child.root = path.resolve(this.host.projectRoot, srcPath)
    child.targets = descriptor.targets
    return child
  }

  includeNode(name, srcPath, extraOptions) {
    if (typeof srcPath !== 'string' || srcPath.length === 0) {
      throw this.host._error('SOME_PATH_NOT_FOUND', `module path does not exist: ${String(srcPath)}`)
    }
    if (path.isAbsolute(srcPath)) {
      throw this.host._error('THE_SRCPATH_IS_NOT_A_RELATIVE_PATH', `module srcPath must be relative: ${srcPath}`)
    }
    const root = path.resolve(this.host.projectRoot, srcPath)
    if (!fs.existsSync(root)) {
      throw this.host._error('SOME_PATH_NOT_FOUND', `module path does not exist: ${srcPath}`)
    }
    this.children.push(this._child(name, srcPath, { extraOptions }))
  }

  excludeNodeByName(name) {
    if (!this.children.some((descriptor) => descriptor.name === name)) {
      this.host._warn('HVIGOR_CONFIG_NODE_NOT_FOUND', `The Node to exclude '${name}' was not found.`)
    }
    this.children = this.children.filter((descriptor) => descriptor.name !== name)
    this.rootDescriptor.children = this.children
  }

  getAllNodeDescriptor() {
    return this.children
  }

  getRootNodeDescriptor() {
    return this.rootDescriptor
  }

  getNodeDescriptorByName(name) {
    return this.children.find((descriptor) => descriptor.name === name)
  }

  applyToHost() {
    this.host._moduleDescriptors = this.children.map((descriptor) => ({
      name: descriptor.name,
      srcPath: descriptor.srcPath,
      root: descriptor.root,
      targets: descriptor.targets,
      extraOptions: Object.fromEntries(descriptor.extraOptions),
      moduleProfile: this.host._readBuildProfileFile(path.join(descriptor.root, 'build-profile.json5'), 'module') || {},
    }))
  }
}

class HvigorBuildResult {
  constructor(error, report) {
    this.error = error
    this.report = report
  }

  getError() {
    return this.error
  }

  getReportJson() {
    return this.report
  }
}

/**
 * Runtime host for the two hvigorfile plugin layers:
 *
 * 1. bind the exported OHOS system plugin and expose its node context;
 * 2. bind and execute TypeScript HvigorPlugin objects (`pluginId`, `context`,
 *    `apply(node)`) after each node is evaluated.
 *
 * The ordering follows @ohos/hvigor 6.24.3
 * `src/base/internal/lifecycle/configuration.js`: project first, modules in
 * sequence, system binding before custom-plugin application, then the global
 * `nodesEvaluated` hook. Package-bound tasks use the dependency direction from
 * `src/base/vigor/task/impl/transform-hvigor-task.js`.
 */
class HvigorRuntimeHost {
  constructor(projectRoot = process.cwd(), config = {}) {
    this.projectRoot = path.resolve(projectRoot)
    this.config = config || {}
    this._nodesByFile = new Map()
    this._nodesByName = new Map()
    this._configEvaluatedCallbacks = []
    this._nodesInitializedCallbacks = []
    this._beforeNodeEvaluateCallbacks = []
    this._afterNodeEvaluateCallbacks = []
    this._nodesEvaluatedCallbacks = []
    this._taskGraphResolvedCallbacks = []
    this._buildFinishedCallbacks = []
    this._watchWorkerMessageCallbacks = []
    this._fallbackEffects = this._newEffects()
    this._effects = this._fallbackEffects
    this._activeFile = null
    this.hvigorWorkspaceNodeModules = null
    this._projectProfile =
      this._readBuildProfileFile(path.join(this.projectRoot, 'build-profile.json5'), 'project') || {}
    this._projectPackage = this._readObjectFile(path.join(this.projectRoot, 'oh-package.json5')) || {}
    this._appJson = this._readObjectFile(path.join(this.projectRoot, 'AppScope', 'app.json5'))
    this._selectedProductName = this.config.product || this._projectProfile.app?.products?.[0]?.name || 'default'
    this._moduleDescriptors = toArray(this._projectProfile.modules).map((module) => ({
      ...module,
      name: module.name || path.basename(module.srcPath || ''),
      root: path.resolve(this.projectRoot, module.srcPath || module.name || ''),
    }))
    this._hvigorConfig = new HvigorConfigView(this)
    this._parameter = this._makeParameter()
    this._commandEntryTasks = toArray(this.config.tasks ?? this.config.task)
  }

  _newEffects() {
    return {
      compilePluginPath: [],
      hvigorPluginFile: [],
      target: null,
      buildOption: {},
    }
  }

  moduleRoots() {
    return this._moduleDescriptors.map((module) => module.root)
  }

  async evaluateHvigorConfig() {
    for (const fileName of ['hvigorconfig.ts', 'hvigorconfig.js', 'hvigorconfig.cjs', 'hvigorconfig.mjs']) {
      const file = path.join(this.projectRoot, fileName)
      if (fs.existsSync(file)) {
        this._activeFile = file
        await loadModule(file, this)
        break
      }
    }
    for (const callback of this._configEvaluatedCallbacks.splice(0)) {
      await callback(this._hvigorConfig)
    }
    this._hvigorConfig.applyToHost()
  }

  async runNodesInitialized() {
    const callbacks = this._nodesInitializedCallbacks.splice(0)
    const hvigor = this._makeHvigorMock(this._activeFile).hvigor
    for (const callback of callbacks) {
      await callback(hvigor)
    }
  }

  async runTaskGraphResolved() {
    const callbacks = this._taskGraphResolvedCallbacks.splice(0)
    const hvigor = this._makeHvigorMock(this._activeFile).hvigor
    for (const callback of callbacks) {
      await callback(hvigor)
    }
  }

  async runBuildFinished(error, report = null) {
    const callbacks = this._buildFinishedCallbacks.splice(0)
    const result = new HvigorBuildResult(error, report)
    for (const callback of callbacks) {
      await callback(result)
    }
  }

  async runWatchWorkerMessage(message) {
    for (const callback of this._watchWorkerMessageCallbacks) {
      await callback(message)
    }
  }

  _readObjectFile(file) {
    if (!fs.existsSync(file)) {
      return undefined
    }
    try {
      const text = fs.readFileSync(file, 'utf8')
      return Function(`"use strict"; return (${text}\n);`)()
    } catch (cause) {
      throw this._error('INVALID_HVIGOR_MODEL_FILE', `failed to evaluate ${file}: ${cause.message}`, { cause })
    }
  }

  _readBuildProfileFile(file, profileKind) {
    const profile = this._readObjectFile(file)
    if (!profile || typeof profile !== 'object') {
      return profile
    }

    if (profileKind === 'project') {
      for (const [index, product] of toArray(profile.app?.products).entries()) {
        this.assertNoDeclarativeCompilePluginFile(product?.buildOption, file, `app.products[${index}].buildOption`)
      }
      for (const [index, buildMode] of toArray(profile.app?.buildModeSet).entries()) {
        this.assertNoDeclarativeCompilePluginFile(
          buildMode?.buildOption,
          file,
          `app.buildModeSet[${index}].buildOption`,
        )
      }
      return profile
    }

    this.assertNoDeclarativeCompilePluginFile(profile.buildOption, file, 'buildOption')
    for (const [index, buildOption] of toArray(profile.buildOptionSet).entries()) {
      this.assertNoDeclarativeCompilePluginFile(buildOption, file, `buildOptionSet[${index}]`)
    }
    for (const [index, target] of toArray(profile.targets).entries()) {
      this.assertNoDeclarativeCompilePluginFile(
        target?.config?.buildOption,
        file,
        `targets[${index}].config.buildOption`,
      )
      this.assertNoDeclarativeCompilePluginFile(target?.buildOption, file, `targets[${index}].buildOption`)
    }
    return profile
  }

  assertNoDeclarativeCompilePluginFile(buildOption, sourceFile, fieldPath) {
    const arkOptions = buildOption && typeof buildOption === 'object' ? buildOption.arkOptions : undefined
    if (!arkOptions || typeof arkOptions !== 'object') {
      return
    }
    if (!Object.prototype.hasOwnProperty.call(arkOptions, 'compilePluginFile')) {
      return
    }
    const rejectedField = `${fieldPath}.arkOptions.compilePluginFile`
    throw this._error(
      'HVIGOR_SCHEMA_COMPILE_PLUGIN_FILE',
      `${rejectedField} is not accepted by the latest @ohos/hvigor-ohos-plugin ${HVIGOR_PARITY_VERSION} ` +
        `schema in ${sourceFile}; register the .ts plugin from a module HvigorPlugin with ` +
        'OhosModuleContext.loadCompilePlugin(path).',
      { sourceFile, fieldPath: rejectedField },
    )
  }

  _clone(value) {
    if (value === undefined) {
      return undefined
    }
    return JSON.parse(JSON.stringify(value))
  }

  _deepFreeze(value) {
    if (!value || typeof value !== 'object' || Object.isFrozen(value)) {
      return value
    }
    for (const child of Object.values(value)) {
      this._deepFreeze(child)
    }
    return Object.freeze(value)
  }

  _freezeClone(value) {
    return this._deepFreeze(this._clone(value))
  }

  _makeParameter() {
    const properties = {
      ...(this.config.hvigorProperties && typeof this.config.hvigorProperties === 'object'
        ? this.config.hvigorProperties
        : {}),
    }
    const extParams = {
      ...(this.config.extParams && typeof this.config.extParams === 'object' ? this.config.extParams : {}),
    }
    for (const entry of toArray(this.config.prop)) {
      if (typeof entry !== 'string') continue
      const separator = entry.indexOf('=')
      const key = separator < 0 ? entry : entry.slice(0, separator)
      extParams[key === 'coverage' ? 'ohos-test-coverage' : key] =
        separator < 0 ? undefined : entry.slice(separator + 1)
    }
    const startParams = Object.freeze({
      analyze: this.config.analyze || false,
      daemon: Boolean(this.config.daemon),
      parallel: this.config.parallel !== false,
      incremental: this.config.cache !== false,
      logLevel: this.config.logLevel || 'info',
      typeCheck: Boolean(this.config.enableBuildScriptTypeCheck ?? this.config.typeCheck),
      optimizationStrategy: this.config.optimizationStrategy || '',
      hotCompile: Boolean(this.config.hotCompile),
      hotReloadBuild: Boolean(this.config.hotReloadBuild),
    })
    return {
      getProperty: (key) => properties[key],
      setProperty: (key, value) => {
        properties[key] = value
      },
      getProperties: () => Object.freeze({ ...properties }),
      getExtParam: (key) => extParams[key],
      getExtParams: () => Object.freeze({ ...extParams }),
      getStartParams: () => startParams,
      getWorkspaceDir: () => process.env.WORKSPACE_DIR || this.projectRoot,
    }
  }

  _error(code, message, extra = {}) {
    const error = new Error(message, extra.cause ? { cause: extra.cause } : undefined)
    error.code = code
    for (const [key, value] of Object.entries(extra)) {
      if (key !== 'cause') {
        error[key] = value
      }
    }
    return error
  }

  _warn(code, message) {
    emitWarningDiagnostic({ code, message })
  }

  prepareHvigorfiles(files) {
    for (const file of files) {
      this._nodeForFile(file, true)
    }
    const projectNode =
      this._nodesByFile.get(this._normalizeFile(path.join(this.projectRoot, 'hvigorfile.ts'))) ||
      Array.from(this._nodesByFile.values()).find((node) => node.root === this.projectRoot)
    if (projectNode) {
      for (const node of this._nodesByFile.values()) {
        if (node !== projectNode) {
          node.parent = projectNode
          projectNode.children.push(node)
        }
      }
    }
  }

  async beginHvigorfile(file) {
    const node = this._nodeForFile(file, true)
    this._activeFile = node.file
    this._effects = node.effects
    for (const callback of this._beforeNodeEvaluateCallbacks) {
      await callback(node.wrapper)
    }
    for (const callback of node.beforeNodeEvaluateCallbacks.splice(0)) {
      await callback.call(node.wrapper, node.wrapper)
    }
    return node.wrapper
  }

  effectsFor(file) {
    return this._nodeForFile(file, true).effects
  }

  bindHvigorfile(file, value) {
    const node = this._nodeForFile(file, true)
    node.loaderConfig = value && typeof value === 'object' ? value.config : undefined
    node.exportValue = value
    const system = value && typeof value === 'object' && value.system ? value.system : value
    const kind = system && system.__arkdownHvigorSystemKind
    if (!kind) {
      return
    }
    node.kind = kind
    const pluginId = this._pluginIdForKind(kind)
    const systemPlugin = {
      getPluginId: () => pluginId,
      getContext: () => this._ohosContextFor(node),
    }
    node.systemPlugins.set(pluginId, systemPlugin)
    node.contextFactories.set(pluginId, systemPlugin.getContext)
    node.pluginIds.push(pluginId)
  }

  async applyCustomPlugins(file, plugins) {
    const node = this._nodeForFile(file, true)
    if (!plugins || (Array.isArray(plugins) && plugins.length === 0)) {
      return
    }
    if (!Array.isArray(plugins)) {
      this._warn('INVALID_HVIGOR_CUSTOM_PLUGINS', `invalid custom plugins config in ${file}; expected an array`)
      return
    }

    const validPlugins = []
    for (let index = 0; index < plugins.length; index++) {
      const plugin = plugins[index]
      if (!plugin || typeof plugin !== 'object' || typeof plugin.apply !== 'function') {
        this._warn('INVALID_HVIGOR_CUSTOM_PLUGIN', `plugins[${index}] in ${file} must contain an apply(node) function`)
        continue
      }
      if (typeof plugin.pluginId !== 'string') {
        this._warn('INVALID_HVIGOR_CUSTOM_PLUGIN_ID', `plugins[${index}] in ${file} must contain a string pluginId`)
      }
      if (plugin.context) {
        // hvigor 6.24.3 binds context before any plugin apply callback runs.
        // Calling `.bind` intentionally preserves hvigor's failure for a
        // truthy non-function context declaration.
        node.contextFactories.set(plugin.pluginId, plugin.context.bind(plugin))
        node.pluginIds.push(plugin.pluginId)
      }
      validPlugins.push(plugin)
    }

    for (const plugin of validPlugins) {
      try {
        await plugin.apply(node.wrapper)
      } catch (cause) {
        throw this._error(
          'HVIGOR_CUSTOM_PLUGIN_APPLY_FAILED',
          `custom plugin ${JSON.stringify(plugin.pluginId)} failed while applying to ${node.name}: ${cause.message}`,
          { cause, pluginId: plugin.pluginId, hvigorfile: file },
        )
      }
    }
  }

  async runAfterNodeEvaluate(file) {
    const node = this._nodeForFile(file, true)
    for (const callback of this._afterNodeEvaluateCallbacks) {
      await callback(node.wrapper)
    }
    for (const callback of node.afterNodeEvaluateCallbacks.splice(0)) {
      await callback.call(node.wrapper, node.wrapper)
    }
  }

  async runNodesEvaluated() {
    const callbacks = this._nodesEvaluatedCallbacks.splice(0)
    const hvigor = this._makeHvigorMock(this._activeFile).hvigor
    for (const callback of callbacks) {
      await callback(hvigor)
    }
  }

  _normalizeFile(file) {
    const resolved = path.resolve(file)
    for (const extension of ['.ts', '.mjs', '.cjs', '.js']) {
      if (resolved.endsWith(extension)) {
        return resolved.slice(0, -extension.length)
      }
    }
    return resolved
  }

  _nodeForFile(file, create = false) {
    const normalized = this._normalizeFile(file)
    let node = this._nodesByFile.get(normalized)
    if (node || !create) {
      return node
    }
    const actualFile = path.resolve(file)
    const root = path.dirname(actualFile)
    const descriptor = this._moduleDescriptors.find((module) => module.root === root)
    const isProject = root === this.projectRoot
    const name = descriptor?.name || (isProject ? path.basename(this.projectRoot) : path.basename(root))
    node = {
      file: actualFile,
      root,
      name,
      isProject,
      descriptor,
      kind: null,
      parent: null,
      children: [],
      effects: this._newEffects(),
      contextFactories: new Map(),
      systemPlugins: new Map(),
      pluginIds: [],
      tasks: new Map(),
      extraOptions: new Map(Object.entries(descriptor?.extraOptions || {})),
      beforeNodeEvaluateCallbacks: [],
      afterNodeEvaluateCallbacks: [],
      loaderConfig: undefined,
      exportValue: undefined,
      moduleProfile: isProject
        ? this._projectProfile
        : descriptor?.moduleProfile ||
          this._readBuildProfileFile(path.join(root, 'build-profile.json5'), 'module') ||
          {},
      moduleJson: this._readObjectFile(path.join(root, 'src', 'main', 'module.json5')),
      modulePackage: this._readObjectFile(path.join(root, 'oh-package.json5')) || {},
    }
    node.wrapper = this._makeNodeWrapper(node)
    this._nodesByFile.set(normalized, node)
    this._nodesByName.set(name, node)
    return node
  }

  _makeNodeWrapper(node) {
    const normalizedFile = new HvigorNormalizedFile(node.root)
    const wrapper = {
      nodeDir: normalizedFile,
      classKind: node.isProject ? 'project' : 'node',
      getNodeDir: () => normalizedFile,
      getNodeName: () => node.name,
      getNodePath: () => node.root,
      getParentNode: () => node.parent?.wrapper,
      subNodes: (callback) => node.children.forEach((child) => callback(child.wrapper)),
      getSubNodeByName: (name) => node.children.find((child) => child.name === name)?.wrapper,
      getContext: (pluginId) => {
        const factory = node.contextFactories.get(pluginId)
        return typeof factory === 'function' ? factory() : undefined
      },
      getAllPluginIds: () => [...node.pluginIds],
      getConfigOpt: () => this._makeConfig(node),
      addExtraOption: (key, value) => node.extraOptions.set(key, value),
      getExtraOption: (key) => node.extraOptions.get(key),
      registerTask: (task) => this._registerTask(node, task),
      getTaskByName: (name) => {
        const task = node.tasks.get(name)
        if (!task) {
          this._warn('HVIGOR_TASK_NOT_FOUND', `The task '${name}' cannot be found on the current node.`)
        }
        return task?.wrapper
      },
      getAllTasks: () => Array.from(node.tasks.values(), (task) => task.wrapper),
      beforeNodeEvaluate: (callback) => node.beforeNodeEvaluateCallbacks.push(callback),
      afterNodeEvaluate: (callback) => node.afterNodeEvaluateCallbacks.push(callback),
      getPluginById: (pluginId) => node.systemPlugins.get(pluginId),
    }
    // getNode(__filename) historically exposed these aliases as well.
    wrapper.getName = wrapper.getNodeName
    return wrapper
  }

  _makeConfig(node) {
    return {
      getObject: (name) => node.loaderConfig?.[name],
      setObject: (name, value) => {
        if (!node.loaderConfig || typeof node.loaderConfig !== 'object') {
          node.loaderConfig = {}
        }
        node.loaderConfig[name] = value
      },
    }
  }

  _pluginIdForKind(kind) {
    return {
      app: 'com.ohos.app',
      hap: 'com.ohos.hap',
      har: 'com.ohos.har',
      hsp: 'com.ohos.hsp',
    }[kind]
  }

  _moduleTypeForKind(kind) {
    return { hap: 'entry', har: 'har', hsp: 'shared' }[kind] || kind
  }

  _targetCandidatesFor(node) {
    const descriptorTargets = toArray(node.descriptor?.targets)
    const moduleTargets = toArray(node.moduleProfile?.targets)
    return descriptorTargets.length > 0 ? descriptorTargets : moduleTargets
  }

  _targetApplies(target) {
    const products = toArray(target.applyToProducts)
    const productMatches =
      products.length === 0 ? this._selectedProductName === 'default' : products.includes(this._selectedProductName)
    if (!productMatches) {
      return false
    }
    const modes = toArray(target.buildModeSet)
    return modes.length === 0 || modes.some((mode) => mode?.name === (this.config.mode || 'debug'))
  }

  _effectiveTargetFor(node) {
    const candidates = this._targetCandidatesFor(node)
    const productMatches = (target) => {
      const products = toArray(target.applyToProducts)
      return products.length === 0
        ? this._selectedProductName === 'default'
        : products.includes(this._selectedProductName)
    }
    const mode = this.config.mode || 'debug'
    return (
      candidates.find(
        (target) =>
          toArray(target.buildModeSet).length > 0 &&
          toArray(target.buildModeSet).some((entry) => entry?.name === mode) &&
          productMatches(target),
      ) ||
      candidates.find((target) => target.name === 'default' && productMatches(target)) ||
      candidates.find(
        (target) => productMatches(target) && toArray(target.buildModeSet).length === 0,
      ) ||
      candidates.find((target) => target.name === 'default')
    )
  }

  _targetEntriesFor(node) {
    if (this.config.module && this.config.module !== node.name) {
      return []
    }
    const candidates = this._targetCandidatesFor(node)
    const applicable = candidates.filter((target) => this._targetApplies(target))
    if (this.config.buildTargetName) {
      const requested = applicable.find((target) => target.name === this.config.buildTargetName)
      return [requested || this._effectiveTargetFor(node) || { name: 'default' }]
    }
    if (candidates.length === 0) {
      return [{ name: 'default' }]
    }
    return applicable
  }

  compilePluginRecords(moduleOverrides = new Map()) {
    const records = []
    const nodes = this._moduleDescriptors.map((descriptor) => {
      const existingNode = Array.from(this._nodesByFile.values()).find(
        (candidate) => candidate.root === descriptor.root,
      )
      return (
        existingNode || {
          name: descriptor.name,
          root: descriptor.root,
          descriptor,
          moduleProfile: descriptor.moduleProfile || {},
        }
      )
    })
    const knownRoots = new Set(nodes.map((node) => path.resolve(node.root)))
    for (const node of this._nodesByFile.values()) {
      const root = path.resolve(node.root)
      if (!node.isProject && !knownRoots.has(root)) {
        nodes.push(node)
        knownRoots.add(root)
      }
    }
    for (const node of nodes) {
      if (this.config.module && this.config.module !== node.name) {
        continue
      }
      const targets = [...this._targetEntriesFor(node)]
      if (!this.config.buildTargetName) {
        const effective = this._effectiveTargetFor(node)
        if (effective && !targets.some((target) => target.name === effective.name)) {
          targets.push(effective)
        }
      }
      if (targets.length === 0) {
        targets.push({ name: 'default' })
      }
      for (const target of targets) {
        const effective = mergeHvigorConfig(
          this._buildOptionFor(node, target),
          moduleOverrides.get(path.resolve(node.root)),
        )
        const configured = effective?.arkOptions?.compilePluginFile
        if (configured === undefined) {
          continue
        }
        records.push({
          file: this._resolveCompilePluginFile(node, configured),
          moduleRoot: node.root,
          targetName: target.name,
        })
      }
    }
    return records
  }

  _resolveCompilePluginFile(node, configured) {
    if (typeof configured !== 'string' || configured.length === 0) {
      throw this._error(
        'INVALID_COMPILE_PLUGIN_FILE',
        `'compilePluginFile' must be a non-empty string in ${path.join(node.root, 'build-profile.json5')}.`,
      )
    }
    const requiredPrefix = './build/hvigor_compile/'
    if (!configured.startsWith(requiredPrefix)) {
      throw this._error(
        'INVALID_COMPILE_PLUGIN_PREFIX',
        `'compilePluginFile' must start with '${requiredPrefix}'.`,
        { pluginPath: configured, moduleName: node.name },
      )
    }
    const compileRoot = path.resolve(node.root, 'build', 'hvigor_compile')
    let resolved = path.resolve(node.root, configured)
    const relative = path.relative(compileRoot, resolved)
    if (relative === '' || relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
      throw this._error(
        'COMPILE_PLUGIN_OUTSIDE_HVIGOR_DIRECTORY',
        `'compilePluginFile' is not under directory: '${compileRoot}'.`,
        { pluginPath: resolved, moduleName: node.name },
      )
    }
    const fileName = path.basename(resolved)
    const suffixIndex = fileName.lastIndexOf('.')
    if (suffixIndex > 0 && fileName.slice(suffixIndex) !== '.ts') {
      throw this._error(
        'PLUGIN_FILE_MUST_END_WITH_TS',
        `'compilePluginFile' must end with .ts.`,
        { pluginPath: resolved, moduleName: node.name },
      )
    }
    if (!fs.existsSync(resolved)) {
      resolved = `${resolved}.ts`
    }
    if (!fs.existsSync(resolved)) {
      throw this._error('PLUGIN_PATH_NOT_FOUND', `'compilePluginFile' does not exist.`, {
        pluginPath: resolved,
        moduleName: node.name,
      })
    }
    return resolved
  }

  _selectedProduct() {
    const product = toArray(this._projectProfile.app?.products).find(
      (candidate) => candidate.name === this._selectedProductName,
    ) || { name: this._selectedProductName }
    return {
      getProductName: () => product.name,
      getBundleType: () => product.bundleType || 'app',
      getBundleName: () => product.bundleName,
      getOutputArtifactName: () => product.output?.artifactName,
    }
  }

  _buildOptionNameFor(node, targetName) {
    const buildMode = this.config.mode || 'debug'
    for (const binder of toArray(node.moduleProfile?.buildModeBinder).slice().reverse()) {
      if (binder.buildModeName !== buildMode) {
        continue
      }
      const mapping = toArray(binder.mappings)
        .slice()
        .reverse()
        .find((candidate) => candidate.targetName === targetName)
      if (typeof mapping?.buildOptionName === 'string' && mapping.buildOptionName.length > 0) {
        return mapping.buildOptionName
      }
    }
    if (buildMode === 'debug' || buildMode === 'test') {
      return 'debug'
    }
    return buildMode === 'release' ? 'release' : 'default'
  }

  _buildOptionSetFor(node, name, stack = []) {
    const option = toArray(node.moduleProfile?.buildOptionSet)
      .slice()
      .reverse()
      .find((candidate) => candidate.name === name)
    if (!option) {
      return undefined
    }
    if (stack.includes(name)) {
      throw this._error(
        'CYCLIC_HVIGOR_BUILD_OPTION_SET',
        `circular buildOptionSet copyFrom chain: ${[...stack, name].join(' -> ')}`,
      )
    }
    const parent =
      typeof option.copyFrom === 'string' ? this._buildOptionSetFor(node, option.copyFrom, [...stack, name]) : undefined
    const { name: _name, copyFrom: _copyFrom, ...buildOption } = option
    return mergeHvigorConfig(parent, this._clone(buildOption))
  }

  _moduleBuildOptionFor(node, targetName) {
    const target = toArray(node.moduleProfile?.targets)
      .slice()
      .reverse()
      .find((candidate) => candidate.name === targetName)
    return mergeHvigorConfig(
      mergeHvigorConfig(
        this._clone(node.moduleProfile?.buildOption) || {},
        this._buildOptionSetFor(node, this._buildOptionNameFor(node, targetName)),
      ),
      this._clone(target?.config?.buildOption ?? target?.buildOption),
    )
  }

  _buildOptionFor(node, target) {
    const buildMode = this.config.mode || 'debug'
    const modeOption = toArray(this._projectProfile.app?.buildModeSet)
      .slice()
      .reverse()
      .find((candidate) => candidate.name === buildMode)?.buildOption
    const product = toArray(this._projectProfile.app?.products)
      .slice()
      .reverse()
      .find((candidate) => candidate.name === this._selectedProductName)
    return mergeHvigorConfig(
      mergeHvigorConfig(this._clone(modeOption) || {}, this._clone(product?.buildOption) || {}),
      this._moduleBuildOptionFor(node, target.name),
    )
  }

  _targetFor(node, target) {
    const moduleBuildPath = path.join(node.root, 'build', this._selectedProductName)
    return {
      getTargetName: () => target.name,
      getCurrentProduct: () => this._selectedProduct(),
      // ModuleTargetImpl._getBuildOption() in hvigor 6.24.3 returns a cloned,
      // deeply frozen snapshot. Mutations must go through the module context.
      getBuildOption: () => this._freezeClone(this._buildOptionFor(node, target)),
      getBuildTargetOutputPath: () => path.join(moduleBuildPath, 'outputs', target.name),
      getModulePathDetails: () => ({
        getModuleBuildPath: () => moduleBuildPath,
        getIntermediatesRes: () => path.join(moduleBuildPath, 'intermediates', 'res'),
        getModuleBuildCachePath: () => path.join(moduleBuildPath, 'cache'),
      }),
    }
  }

  _packageValue(packageObject, key) {
    return this._clone(packageObject?.[key])
  }

  _unsupportedContextApi(api, reason) {
    throw this._error('ARKDOWN_UNSUPPORTED_HVIGOR_CONTEXT_API', `${api} is not supported: ${reason}`, { api })
  }

  _targetArtifactNames(node) {
    return new Map(
      this._targetEntriesFor(node)
        .filter((target) => typeof target.output?.artifactName === 'string')
        .map((target) => [target.name, target.output.artifactName]),
    )
  }

  _sdkDetails() {
    const sdkDir =
      this.config.sdkPath ||
      process.env.OHOS_SDK_HOME ||
      process.env.DEVECO_SDK_HOME ||
      path.dirname(this.config.etsLoaderPath || '')
    const sdkVersion = this.config.compileSdkVersion || this.config.targetSdkVersion || ''
    return {
      isOhos: () => this.config.runtimeOS !== 'HarmonyOS',
      getSdkDir: () => sdkDir,
      getSdkVersion: () => sdkVersion,
      getEtsComponentVersion: () => this.config.etsComponentVersion || sdkVersion,
      getEtsComponentReleaseType: () => this.config.etsComponentReleaseType || 'Release',
    }
  }

  _ohosContextFor(node) {
    if (node.ohosContext) {
      return node.ohosContext
    }
    if (node.kind === 'app') {
      const buildRoot = path.join(node.root, 'build')
      node.ohosContext = {
        getProjectName: () => node.name,
        getProjectPath: () => node.root,
        getBuildRootPath: () => buildRoot,
        getBuildProductOutputPath: () => path.join(buildRoot, 'outputs', this._selectedProductName),
        getBuildMode: () => this.config.mode || 'debug',
        getBuildProfileOpt: () => this._clone(this._projectProfile),
        getCurrentProduct: () => this._selectedProduct(),
        getAppJsonOpt: () => this._clone(this._appJson),
        getDependenciesOpt: () => this._packageValue(this._projectPackage, 'dependencies'),
        getDevDependenciesOpt: () => this._packageValue(this._projectPackage, 'devDependencies'),
        getDynamicDependenciesOpt: () => this._packageValue(this._projectPackage, 'dynamicDependencies'),
        getOverrides: () => this._packageValue(this._projectPackage, 'overrides'),
        getSdkDetails: () => this._sdkDetails(),
        isDebugLineEnable: () => String(this._parameter.getExtParam('ohos-debug-line')) === 'true',
        getFastBuildApp: () => Boolean(this._projectProfile.app?.buildOption?.packOptions?.fastBuildApp),
        getAppWithSignedPkg: () => Boolean(this._projectProfile.app?.buildOption?.packOptions?.appWithSignedPkg),
        getOhpmDependencyInfo: () =>
          this._unsupportedContextApi(
            'OhosProjectContext.getOhpmDependencyInfo',
            'the typed OHPM dependency graph is not yet exposed to the JavaScript runtime',
          ),
        getOhpmRemoteHspDependencyInfo: () =>
          this._unsupportedContextApi(
            'OhosProjectContext.getOhpmRemoteHspDependencyInfo',
            'remote HSP resolution is owned by the Rust dependency snapshot',
          ),
        getAllModuleNameHash: () =>
          this._unsupportedContextApi(
            'OhosProjectContext.getAllModuleNameHash',
            'hvigor-compatible module graph hashing is not yet implemented',
          ),
        setBuildProfileOpt: () =>
          this._unsupportedContextApi(
            'OhosProjectContext.setBuildProfileOpt',
            'project profile mutations are not yet forwarded into the immutable Rust build snapshot',
          ),
        setAppJsonOpt: () =>
          this._unsupportedContextApi(
            'OhosProjectContext.setAppJsonOpt',
            'AppScope mutations are not yet forwarded into the immutable Rust build snapshot',
          ),
        setDependenciesOpt: () => this._unsupportedPackageMutation('OhosProjectContext.setDependenciesOpt'),
        setDevDependenciesOpt: () => this._unsupportedPackageMutation('OhosProjectContext.setDevDependenciesOpt'),
        setDynamicDependenciesOpt: () =>
          this._unsupportedPackageMutation('OhosProjectContext.setDynamicDependenciesOpt'),
        setOverrides: () => this._unsupportedPackageMutation('OhosProjectContext.setOverrides'),
      }
      return node.ohosContext
    }

    const artifactNames = this._targetArtifactNames(node)
    node.ohosContext = {
      getModuleName: () => node.name,
      getModulePath: () => node.root,
      getModuleType: () => this._moduleTypeForKind(node.kind),
      getBuildProductRootPath: () => path.join(node.root, 'build', this._selectedProductName),
      getBuildMode: () => this.config.mode || 'debug',
      targets: (callback) => this._targetEntriesFor(node).forEach((target) => callback(this._targetFor(node, target))),
      getTargetArtifactNames: () => new Map(artifactNames),
      getTargetArtifactName: (targetName) => artifactNames.get(targetName),
      getBuildProfileOpt: () => this._clone(node.moduleProfile),
      setBuildProfileOpt: (profile) => {
        node.moduleProfile = this._clone(profile) || {}
        const selectedTarget = this._targetEntriesFor(node)[0] || { name: this.config.buildTargetName || 'default' }
        node.effects.buildOption = mergeHvigorConfig(
          node.effects.buildOption,
          this._moduleBuildOptionFor(node, selectedTarget.name),
        )
      },
      getModuleJsonOpt: () => this._clone(node.moduleJson),
      setModuleJsonOpt: () => {
        throw this._error(
          'ARKDOWN_UNSUPPORTED_HVIGOR_CONTEXT_API',
          'setModuleJsonOpt is not supported because arkdown build sessions use immutable manifest snapshots',
        )
      },
      getDependenciesOpt: () => this._packageValue(node.modulePackage, 'dependencies'),
      getDevDependenciesOpt: () => this._packageValue(node.modulePackage, 'devDependencies'),
      getDynamicDependenciesOpt: () => this._packageValue(node.modulePackage, 'dynamicDependencies'),
      getVersion: () => node.modulePackage?.version,
      getOhpmDependencyInfo: () =>
        this._unsupportedContextApi(
          'OhosModuleContext.getOhpmDependencyInfo',
          'the typed OHPM dependency graph is not yet exposed to the JavaScript runtime',
        ),
      getOhpmRemoteHspDependencyInfo: () =>
        this._unsupportedContextApi(
          'OhosModuleContext.getOhpmRemoteHspDependencyInfo',
          'remote HSP resolution is owned by the Rust dependency snapshot',
        ),
      setDependenciesOpt: () => this._unsupportedPackageMutation('OhosModuleContext.setDependenciesOpt'),
      setDevDependenciesOpt: () => this._unsupportedPackageMutation('OhosModuleContext.setDevDependenciesOpt'),
      setDynamicDependenciesOpt: () => this._unsupportedPackageMutation('OhosModuleContext.setDynamicDependenciesOpt'),
      setVersion: () => this._unsupportedPackageMutation('OhosModuleContext.setVersion'),
      loadCompilePlugin: (pluginPath) => this._loadCompilePlugin(node, pluginPath),
      transformAbc: () =>
        this._unsupportedContextApi(
          'OhosModuleContext.transformAbc',
          'ABC callback execution has not been attached to the Rust bytecode stage',
        ),
    }
    return node.ohosContext
  }

  _unsupportedPackageMutation(api) {
    return this._unsupportedContextApi(
      api,
      'package mutations are not yet forwarded into the immutable Rust dependency snapshot',
    )
  }

  _typeCheckCompilePlugin(pluginFile) {
    let ts
    try {
      ts = require('typescript')
    } catch (cause) {
      throw this._error(
        'HVIGOR_TYPESCRIPT_NOT_FOUND',
        `type-checking compile plugin ${pluginFile} requires TypeScript ${cause.message}`,
        { cause, pluginPath: pluginFile },
      )
    }
    const baseUrl =
      this.hvigorWorkspaceNodeModules || path.resolve(process.env.WORKSPACE_DIR || this.projectRoot, 'node_modules')
    const program = ts.createProgram([pluginFile], {
      allowJs: true,
      alwaysStrict: false,
      baseUrl,
      declaration: false,
      esModuleInterop: false,
      forceConsistentCasingInFileNames: false,
      module: ts.ModuleKind.CommonJS,
      noEmit: true,
      noImplicitAny: false,
      noImplicitReturns: false,
      skipLibCheck: true,
      strict: false,
      target: ts.ScriptTarget.ES2016,
      types: [],
    })
    const diagnostics = ts.getPreEmitDiagnostics(program)
    if (diagnostics.length === 0) {
      return
    }
    const formatHost = {
      getCanonicalFileName: (fileName) => fileName,
      getCurrentDirectory: () => process.cwd(),
      getNewLine: () => ts.sys.newLine,
    }
    const error = this._error(
      'TYPE_ERRORS',
      `TypeScript errors in compile plugin ${pluginFile}:${ts.sys.newLine}${ts.formatDiagnostics(diagnostics, formatHost)}`,
      { pluginPath: pluginFile },
    )
    error.diagnostics = diagnostics.map((diagnostic) => {
      const location =
        diagnostic.file && diagnostic.start !== undefined
          ? diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start)
          : undefined
      return {
        category: ts.DiagnosticCategory[diagnostic.category].toLowerCase(),
        code: diagnostic.code,
        message: ts.flattenDiagnosticMessageText(diagnostic.messageText, ts.sys.newLine),
        file: diagnostic.file?.fileName,
        line: location ? location.line + 1 : undefined,
        column: location ? location.character + 1 : undefined,
      }
    })
    throw error
  }

  _loadCompilePlugin(node, pluginPath) {
    if (typeof pluginPath !== 'string' || pluginPath.length === 0) {
      throw this._error('EMPTY_COMPILE_PLUGIN_PATH', 'compile plugin path must not be empty')
    }
    const resolved = path.isAbsolute(pluginPath) ? pluginPath : path.resolve(node.root, pluginPath)
    if (!fs.existsSync(resolved)) {
      throw this._error('PLUGIN_PATH_NOT_FOUND', `compile plugin path does not exist: ${resolved}`, {
        pluginPath: resolved,
      })
    }
    if (!resolved.endsWith('.ts')) {
      throw this._error('PLUGIN_FILE_MUST_END_WITH_TS', `compile plugin file must end with .ts: ${resolved}`, {
        pluginPath: resolved,
      })
    }
    // Latest OhosModuleContextImpl calls doCompilePluginTypeCheck before
    // storing the plugin path. Keep the same TypeScript 4.9.5 program options
    // here; execution itself remains in Arkdown's Rolldown driver.
    this._typeCheckCompilePlugin(resolved)
    node.effects.compilePluginPath.push(resolved)
  }

  _registerTask(node, definition) {
    if (!definition || typeof definition !== 'object' || typeof definition.name !== 'string') {
      throw this._error('INVALID_HVIGOR_TASK', `custom task on ${node.name} must contain a string name`)
    }
    const taskContext = {
      ...definition,
      context: definition.context == null ? undefined : definition.context.apply(),
    }
    const input = new HvigorTaskInput()
    const output = new HvigorTaskOutput()
    definition.input?.apply(taskContext, [input])
    definition.output?.apply(taskContext, [output])
    if (node.tasks.has(definition.name)) {
      throw this._error('DUPLICATE_HVIGOR_TASK', `duplicate task ${node.name}:${definition.name}`, {
        taskName: definition.name,
        moduleName: node.name,
      })
    }
    const task = {
      key: `${node.name}:${definition.name}`,
      node,
      definition,
      taskContext,
      runContext: { moduleName: node.name, modulePath: node.root },
      input: input.declarations,
      output: output.declarations,
      enabled: true,
      beforeRunHooks:
        typeof definition.beforeRun === 'function' ? [{ callback: definition.beforeRun, declared: true }] : [],
      afterRunHooks:
        typeof definition.afterRun === 'function' ? [{ callback: definition.afterRun, declared: true }] : [],
    }
    task.wrapper = new HvigorTaskView(task)
    node.tasks.set(definition.name, task)
    return definition
  }

  _taskReference(owner, reference) {
    const separator = reference.indexOf(':')
    const nodeName = separator >= 0 ? reference.slice(0, separator) : owner.node.name
    const taskName = separator >= 0 ? reference.slice(separator + 1) : reference
    const node = nodeName
      ? this._nodesByName.get(nodeName)
      : Array.from(this._nodesByFile.values()).find((item) => item.isProject)
    return { node, taskName, task: node?.tasks.get(taskName) }
  }

  _isPackageTask(node, taskName) {
    const match = /^(.+)@Package(Hap|Hsp|Har)$/.exec(taskName)
    if (!match || !node) {
      return false
    }
    const expectedKind = match[2].toLowerCase()
    return node.kind === expectedKind && this._targetEntriesFor(node).some((target) => target.name === match[1])
  }

  _taskExecutionPlan() {
    const allTasks = Array.from(this._nodesByFile.values()).flatMap((node) => Array.from(node.tasks.values()))
    const roots = new Set()
    const extraDependencies = new Map()

    for (const task of allTasks) {
      for (const reference of Array.isArray(task.definition.postDependencies) ? task.definition.postDependencies : []) {
        const target = this._taskReference(task, reference)
        if (target.task) {
          const dependencies = extraDependencies.get(target.task.key) || []
          dependencies.push(task.key)
          extraDependencies.set(target.task.key, dependencies)
        } else if (this._isPackageTask(target.node || task.node, target.taskName)) {
          roots.add(task.key)
        } else {
          throw this._error(
            'ARKDOWN_UNSUPPORTED_HVIGOR_TASK_POST_DEPENDENCY',
            `task ${task.key} uses unsupported postDependency ${JSON.stringify(reference)}; ` +
              'arkdown supports package-bound custom tasks and custom-task edges',
            { taskName: task.definition.name, dependency: reference },
          )
        }
      }
    }

    const tasksByKey = new Map(allTasks.map((task) => [task.key, task]))
    const visiting = new Set()
    const visited = new Set()
    const plan = []
    const visit = (task) => {
      if (visited.has(task.key)) return
      if (visiting.has(task.key)) {
        throw this._error('CYCLIC_HVIGOR_TASK_DEPENDENCY', `cyclic custom task dependency at ${task.key}`)
      }
      visiting.add(task.key)
      const dependencies = [
        ...(Array.isArray(task.definition.dependencies) ? task.definition.dependencies : []).map((reference) => {
          const dependency = this._taskReference(task, reference).task
          if (!dependency) {
            throw this._error(
              'ARKDOWN_UNSUPPORTED_HVIGOR_TASK_DEPENDENCY',
              `task ${task.key} depends on unsupported task ${JSON.stringify(reference)}`,
              { taskName: task.definition.name, dependency: reference },
            )
          }
          return dependency.key
        }),
        ...(extraDependencies.get(task.key) || []),
      ]
      for (const dependencyKey of dependencies) {
        visit(tasksByKey.get(dependencyKey))
      }
      visiting.delete(task.key)
      visited.add(task.key)
      plan.push(task)
    }
    for (const root of roots) {
      visit(tasksByKey.get(root))
    }
    return plan
  }

  async runPackageBoundCustomTasks() {
    const plan = this._taskExecutionPlan()
    for (const task of plan) {
      if (!task.enabled) {
        continue
      }
      if (typeof task.definition.run !== 'function') {
        throw this._error('INVALID_HVIGOR_TASK_RUN', `task ${task.key} must contain a run function`)
      }
      try {
        for (let index = task.beforeRunHooks.length - 1; index >= 0; index--) {
          const hook = task.beforeRunHooks[index]
          if (hook.declared) {
            await hook.callback.apply(task.taskContext, [task.runContext])
          } else {
            await hook.callback.call(task.wrapper)
          }
        }
        await task.definition.run.apply(task.taskContext, [task.runContext])
        for (const hook of task.afterRunHooks) {
          if (hook.declared) {
            await hook.callback.apply(task.taskContext, [task.runContext])
          } else {
            await hook.callback.call(task.wrapper)
          }
        }
      } catch (cause) {
        throw this._error('HVIGOR_CUSTOM_TASK_FAILED', `custom task ${task.key} failed: ${cause.message}`, {
          cause,
          taskName: task.definition.name,
          moduleName: task.node.name,
        })
      }
    }
  }

  _captureTaskEffect(opts, file, kind) {
    const node = file ? this._nodeForFile(file, true) : null
    const effects = node?.effects || this._effects
    const options = opts || {}
    const ark = options.buildOption?.arkOptions || {}
    for (const candidate of toArray(options.compilePluginPath).concat(toArray(ark.compilePluginPath))) {
      effects.compilePluginPath.push(candidate)
    }
    for (const candidate of toArray(options.hvigorPluginFile).concat(toArray(ark.hvigorPluginFile))) {
      effects.hvigorPluginFile.push(candidate)
    }
    if (options.target) effects.target = options.target
    if (options.buildOption) effects.buildOption = mergeHvigorConfig(effects.buildOption, options.buildOption)
    const plugin = { name: 'arkdown-system-plugin' }
    Object.defineProperty(plugin, '__arkdownHvigorSystemKind', { value: kind || 'hap' })
    return plugin
  }

  _systemFactory(kind, file) {
    const factory = (opts) => this._captureTaskEffect(opts, file, kind)
    Object.defineProperty(factory, '__arkdownHvigorSystemKind', { value: kind })
    return factory
  }

  _makeOhosPluginMock(file = this._activeFile) {
    const OhosPluginId = Object.freeze({
      OHOS_APP_PLUGIN: 'com.ohos.app',
      OHOS_HAP_PLUGIN: 'com.ohos.hap',
      OHOS_HAR_PLUGIN: 'com.ohos.har',
      OHOS_HSP_PLUGIN: 'com.ohos.hsp',
    })
    return {
      OhosPluginId,
      appTasks: this._systemFactory('app', file),
      hapTasks: this._systemFactory('hap', file),
      hspTasks: this._systemFactory('hsp', file),
      harTasks: this._systemFactory('har', file),
    }
  }

  _makeHvigorMock(file = this._activeFile) {
    const host = this
    const registerLifecycle = (name, callbacks, callback) => {
      if (typeof callback !== 'function') {
        throw host._error('INVALID_HVIGOR_LIFECYCLE_CALLBACK', `hvigor.${name} expects a function`)
      }
      callbacks.push(callback)
    }
    const rootNode = () => {
      const node = Array.from(host._nodesByFile.values()).find((candidate) => candidate.isProject)?.wrapper
      if (!node) {
        throw host._error(
          'HVIGOR_CONFIG_FILE_ERROR',
          'The root node is not yet available for build. Register pre-node behavior with hvigor.configEvaluated().',
        )
      }
      return node
    }
    const allNodes = () => {
      const nodes = Array.from(host._nodesByFile.values())
      const root = nodes.find((node) => node.isProject)
      return [...nodes.filter((node) => node !== root), ...(root ? [root] : [])].map((node) => node.wrapper)
    }
    const hvigor = {
      getRootNode: rootNode,
      getAllNodes: allNodes,
      getNodeByName: (name, classKind) =>
        allNodes().find(
          (node) => node.getNodeName() === name && (classKind === undefined || node.classKind === classKind),
        ),
      getHvigorConfig: () => host._hvigorConfig,
      getParameter: () => host._parameter,
      beforeNodeEvaluate: (callback) =>
        registerLifecycle('beforeNodeEvaluate', host._beforeNodeEvaluateCallbacks, callback),
      afterNodeEvaluate: (callback) =>
        registerLifecycle('afterNodeEvaluate', host._afterNodeEvaluateCallbacks, callback),
      configEvaluated: (callback) => registerLifecycle('configEvaluated', host._configEvaluatedCallbacks, callback),
      nodesInitialized: (callback) => registerLifecycle('nodesInitialized', host._nodesInitializedCallbacks, callback),
      nodesEvaluated: (callback) => registerLifecycle('nodesEvaluated', host._nodesEvaluatedCallbacks, callback),
      taskGraphResolved: (callback) =>
        registerLifecycle('taskGraphResolved', host._taskGraphResolvedCallbacks, callback),
      buildFinished: (callback) => registerLifecycle('buildFinished', host._buildFinishedCallbacks, callback),
      onWatchWorkerMessage: (callback) =>
        registerLifecycle('onWatchWorkerMessage', host._watchWorkerMessageCallbacks, callback),
      getCommandEntryTask: () => (host._commandEntryTasks.length > 0 ? [...host._commandEntryTasks] : undefined),
      isCommandEntryTask: (taskName) => host._commandEntryTasks.includes(taskName),
      getExtraConfig: () => new Map(Object.entries(host._parameter.getExtParams())),
      getProject: rootNode,
      getHvigorUserHome,
      getHvigorVersion: () => HVIGOR_PARITY_VERSION,
    }
    return {
      getNode: (scriptPath) => host._nodeForFile(scriptPath || file, false)?.wrapper,
      getHvigorNode: (scriptPath) => host._nodeForFile(scriptPath || file, false)?.wrapper,
      hvigor,
      hvigorConfig: host._hvigorConfig,
      BuildResult: HvigorBuildResult,
      hvigorCore: {
        getHvigorVersion: () => HVIGOR_PARITY_VERSION,
        getProject: rootNode,
        getParameter: () => host._parameter,
        getExtraConfig: () => hvigor.getExtraConfig(),
        getCommandEntryTasks: () => hvigor.getCommandEntryTask(),
        isCommandEntryTask: (taskName) => hvigor.isCommandEntryTask(taskName),
      },
      TaskInput: HvigorTaskInput,
      TaskOutput: HvigorTaskOutput,
    }
  }

  loadWithModuleMocks(file, callback) {
    const originalLoad = Module._load
    const host = this
    Module._load = function arkdownHvigorModuleLoad(request, parent, isMain) {
      if (request === '@ohos/hvigor') {
        return host._makeHvigorMock(parent?.filename || file)
      }
      if (request === '@ohos/hvigor-ohos-plugin') {
        return host._makeOhosPluginMock(parent?.filename || file)
      }
      return originalLoad.call(this, request, parent, isMain)
    }
    try {
      return callback()
    } finally {
      Module._load = originalLoad
    }
  }
}

function collectCompilePluginPaths(value, options = {}) {
  if (!value || typeof value !== 'object') {
    return []
  }
  const includeCompilePluginFile = options.includeCompilePluginFile !== false
  const arkOptions = value.arkOptions || {}
  const buildArkOptions = (value.buildOption && value.buildOption.arkOptions) || {}
  const paths = [
    ...toArray(value.compilePluginPath),
    ...toArray(arkOptions.compilePluginPath),
    ...toArray(buildArkOptions.compilePluginPath),
  ]
  if (includeCompilePluginFile) {
    paths.push(
      ...toArray(value.compilePluginFile),
      ...toArray(arkOptions.compilePluginFile),
      ...toArray(buildArkOptions.compilePluginFile),
    )
  }
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

// normalizeSortedPluginList is the strict hvigor compile-plugin loader used by
// compilePluginPath / hvigorPluginFile. Unlike Arkdown's explicit plugin API,
// hvigor accepts only an object or a flat array of objects and requires a
// non-empty string name even when the plugin declares no hooks.
function normalizeSortedPluginList(value, _baseDir, pluginFile = '<compile-plugin>', moduleRoot, targetName) {
  const exported = unwrapDefault(value)
  const items = Array.isArray(exported) ? exported : [exported]
  return items.map((plugin) => normalizeHvigorCompilePlugin(plugin, pluginFile, moduleRoot, targetName))
}

/**
 * Resolves latest-hvigor `order` values against the latest IDE built-in plugin
 * list, then translates each custom plugin to an absolute slot in Arkdown's
 * reduced Rust-native pipeline. The JavaScript order callback still observes
 * hvigor's names and indices; Rust receives only the resolved numeric slot.
 */
class HvigorCompilePluginPlanner {
  constructor(config = {}) {
    this.config = config
  }

  apply(plugins) {
    const compilePlugins = plugins.filter((plugin) => plugin.__arkdownHvigorCompilePlugin)
    if (compilePlugins.length === 0) {
      return plugins
    }

    const scopes = new Map()
    for (const plugin of compilePlugins) {
      const scope = `${plugin.__arkdownModuleRoot || '*'}\0${plugin.__arkdownTargetName || '*'}`
      const scopedPlugins = scopes.get(scope) || []
      scopedPlugins.push(plugin)
      scopes.set(scope, scopedPlugins)
    }
    const retained = new Set()
    for (const scopedPlugins of scopes.values()) {
      for (const plugin of this._applyScope(scopedPlugins)) {
        retained.add(plugin)
      }
    }
    return plugins.filter((plugin) => !plugin.__arkdownHvigorCompilePlugin || retained.has(plugin))
  }

  _applyScope(compilePlugins) {
    const pipeline = this._builtIns()
    for (const plugin of compilePlugins) {
      const order = plugin.__arkdownHvigorOrder ?? 'last'
      if (order === 'first') {
        pipeline.unshift({ name: plugin.name, plugin })
      } else if (order === 'last') {
        pipeline.push({ name: plugin.name, plugin })
      } else if (typeof order === 'function') {
        const nameToIndex = Object.create(null)
        pipeline.forEach((entry, index) => {
          let name = entry.name
          let suffix = 1
          // arkts-pack.js 6.24.3 tests the stored index by truthiness. Keep
          // that index-zero quirk instead of replacing it with hasOwn().
          while (nameToIndex[name]) {
            name = `${entry.name}_${suffix}`
            suffix++
          }
          nameToIndex[name] = index
        })
        let index
        try {
          index = order(nameToIndex)
        } catch (cause) {
          throw this._orderError(plugin, `order function threw: ${cause.message}`, cause)
        }
        if (typeof index !== 'number' || Number.isNaN(index)) {
          throw this._orderError(plugin, `order function must return a number, got ${JSON.stringify(index)}`)
        }
        pipeline.splice(index < 0 ? 0 : index, 0, { name: plugin.name, plugin })
      }
      // A falsy non-null order such as '' is validated by hvigor but is not
      // inserted by arkts-pack.js. Preserve that observable behavior.
    }

    const duplicate = this._duplicateName(pipeline)
    if (duplicate) {
      const error = new Error(`duplicated Rolldown plugin name '${duplicate}'`)
      error.code = 'DUPLICATED_PLUGIN_NAME_IN_ROLLUP'
      error.pluginName = duplicate
      throw error
    }

    const reduced = pipeline.filter((entry) => entry.plugin || entry.rustName)
    reduced.forEach((entry, index) => {
      if (entry.plugin) {
        Object.defineProperty(entry.plugin, '__arkdownOrderIndex', {
          configurable: true,
          enumerable: true,
          value: index,
        })
      }
    })
    return pipeline.flatMap((entry) => (entry.plugin ? [entry.plugin] : []))
  }

  _orderError(plugin, message, cause) {
    const error = new Error(`Plugin "${plugin.name}": ${message}`, cause ? { cause } : undefined)
    error.code = 'INVALID_PLUGIN_EXPORT'
    error.pluginName = plugin.name
    return error
  }

  _duplicateName(entries) {
    const names = new Set()
    for (const entry of entries) {
      if (names.has(entry.name)) return entry.name
      names.add(entry.name)
    }
    return undefined
  }

  _builtIns() {
    const esmodule = String(this.config.compileMode || 'esmodule').toLowerCase() === 'esmodule'
    const preview = Boolean(this.config.preview)
    const hsp = String(this.config.target || '').toLowerCase() === 'hsp'
    return [
      { name: 'commonPlugin' },
      { name: 'invalidCachePlugin' },
      { name: 'checkDuplicatedFileNamesPlugin', rustName: 'arkdown:duplicate-check' },
      esmodule && { name: 'moduleInfoMetaPlugin', rustName: 'module-meta' },
      { name: 'oh-resolve', rustName: 'ohos-resolve' },
      { name: 'routerMapPlugin' },
      { name: 'newOhmUrlPlugin' },
      { name: 'sensitiveCheckPlugin', rustName: 'import-check' },
      { name: 'commonjs' },
      { name: 'gcPlugin' },
      { name: 'json', rustName: 'json5' },
      { name: 'wrapperChunkPlugin', rustName: 'wrapper-chunk' },
      { name: 'waitAsyncPlugin' },
      { name: 'collectImportersPlugin' },
      hsp && { name: 'hspCollectHarFiles', rustName: 'har-dedup' },
      { name: 'memoryMonitor' },
      { name: 'watchChangedFiles' },
      { name: 'etsChecker' },
      { name: 'etsTransform', rustName: 'ets-transform' },
      { name: 'apiTransform' },
      { name: 'buildInstrument' },
      { name: 'genAbc' },
      { name: 'createProgramPlugin', rustName: preview ? 'minify' : undefined },
      !preview && { name: 'delete', rustName: 'minify' },
    ].filter(Boolean)
  }
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
    // Hvigor's arkts-pack getPlugins passes a Record<string, number>, not a
    // Map. Use a null-prototype record so plugin names such as "constructor"
    // remain ordinary keys.
    const nameToIndex = Object.create(null)
    result.forEach((p, i) => {
      // Deduplicate names by appending _N suffix, matching hvigor behaviour
      let name = p.name || ''
      let suffix = 1
      while (Object.prototype.hasOwnProperty.call(nameToIndex, name)) {
        name = `${p.name || ''}_${suffix}`
        suffix++
      }
      nameToIndex[name] = i
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

function normalizeHvigorCompilePlugin(plugin, pluginFile, moduleRoot, targetName) {
  validateHvigorCompilePlugin(plugin, pluginFile)
  const normalized = normalizeHvigorCompileHookArguments(plugin, pluginFile)
  const internalProperties = {
    __arkdownHvigorCompilePlugin: {
      enumerable: false,
      value: true,
    },
    __arkdownHvigorOrder: {
      enumerable: false,
      value: plugin.order,
    },
  }
  if (moduleRoot) {
    internalProperties.__arkdownModuleRoot = {
      enumerable: false,
      value: path.resolve(moduleRoot),
    }
  }
  if (targetName) {
    internalProperties.__arkdownTargetName = {
      enumerable: false,
      value: targetName,
    }
  }
  Object.defineProperties(normalized, internalProperties)
  return normalized
}

function validateHvigorCompilePlugin(plugin, pluginFile) {
  if (!plugin || typeof plugin !== 'object' || Array.isArray(plugin)) {
    emitDiagnostic('error', {
      code: 'INVALID_PLUGIN_TYPE',
      message: `${pluginFile} must export a compile-plugin object, got ${Array.isArray(plugin) ? 'array' : typeof plugin}`,
    })
  }
  if (!plugin.name) {
    emitDiagnostic('error', {
      code: 'PLUGIN_NAME_NOT_EXIST',
      message: `${pluginFile} exports a compile plugin without a name`,
    })
  }
  if (typeof plugin.name !== 'string') {
    emitDiagnostic('error', {
      code: 'PLUGIN_NAME_NOT_STRING',
      message: `${pluginFile} exports a compile plugin whose name is not a string`,
    })
  }

  for (const key of Object.keys(plugin)) {
    const value = plugin[key]
    if (key === 'name') continue
    if (key === 'order') {
      if (value && value !== 'first' && value !== 'last' && typeof value !== 'function') {
        emitDiagnostic('error', {
          code: 'PLUGIN_NAME_WITH_UNKNOWN_ORDER',
          message: `Plugin "${plugin.name}": unknown order value ${JSON.stringify(value)}`,
          pluginName: plugin.name,
        })
      }
      continue
    }
    if (HVIGOR_COMPILE_HOOKS.has(key)) {
      if (value && typeof value !== 'function') {
        emitDiagnostic('error', {
          code: 'PLUGIN_NAME_WITH_UNKNOWN_HOOK',
          message: `Plugin "${plugin.name}": hook "${key}" must be a function`,
          pluginName: plugin.name,
        })
      }
      continue
    }
    emitDiagnostic('warn', {
      code: 'UNKNOWN_PLUGIN_ATTR',
      message: `'${pluginFile}' exports an invalid plugin '${plugin.name}' with unknown property: '${key}'.`,
      pluginName: plugin.name,
    })
  }
}

function normalizeHvigorCompileHookArguments(plugin, pluginFile) {
  const normalized = { name: plugin.name }
  let pluginShareContext = {}
  const invoke = (hookName, context, args) => {
    try {
      return plugin[hookName].apply(context, args)
    } catch (cause) {
      cause.message = `${cause.message} ${pluginFile}`
      throw cause
    }
  }

  // Internal data bridge only: standard hooks remain native Rolldown hooks.
  // The latest hvigor fork adds `share` to PluginContext; seed the equivalent
  // data before Rolldown starts without exposing another lifecycle hook.
  Object.defineProperty(normalized, '__arkdownSeedHvigorContext', {
    enumerable: false,
    value(contextJson) {
      const hookContext = parsePluginContext(contextJson)
      if (hookContext.share) {
        pluginShareContext = hookContext.share
      }
    },
  })

  if (plugin.buildStart) {
    normalized.buildStart = function buildStart(contextJson) {
      // Installed 6.24.3 arkts-pack.js::wrapperBuildStart invokes the custom
      // hook but deliberately does not return its value to Rollup. Preserve
      // that strict compile-plugin behavior, including an unawaited Promise.
      invoke('buildStart', buildPluginContext(parsePluginContext(contextJson), pluginShareContext), [])
    }
  }
  for (const hookName of ['resolveId', 'load', 'transform']) {
    if (plugin[hookName]) {
      normalized[hookName] = function (...args) {
        return invoke(hookName, buildPluginContext({}, pluginShareContext), args)
      }
    }
  }
  if (plugin.shouldInvalidCache) {
    normalized.shouldInvalidCache = function shouldInvalidCache(request) {
      return invoke('shouldInvalidCache', buildPluginContext({}, pluginShareContext), [
        parseShouldInvalidCacheRequest(request),
      ])
    }
  }
  if (plugin.moduleParsed) {
    normalized.moduleParsed = function moduleParsed(moduleInfo) {
      const parsed = parseModuleInfo(moduleInfo)
      const moduleInfoMap = parsed?.id ? { [parsed.id]: parsed } : {}
      // Matches arkts-pack.js::wrapperModuleParsed: synchronous failures are
      // forwarded, while the user's return value is not returned to Rollup.
      invoke('moduleParsed', buildPluginContext({ moduleInfoMap }, pluginShareContext), [parsed])
    }
  }
  if (plugin.beforeBuildEnd) {
    normalized.beforeBuildEnd = function beforeBuildEnd(contextJson) {
      const hookContext = parsePluginContext(contextJson)
      return invoke('beforeBuildEnd', buildPluginContext(hookContext, pluginShareContext), [hookContext.input])
    }
  }
  if (plugin.buildEnd) {
    normalized.buildEnd = function buildEnd(contextJson) {
      const hookContext = parsePluginContext(contextJson)
      const count = Number(hookContext.errorCount || 0)
      const error = count > 0 ? new Error(`Rolldown build ended with ${count} error(s)`) : undefined
      // Matches arkts-pack.js::wrapperBuildEnd. `beforeBuildEnd` above is the
      // only lifecycle gap wrapper that returns and is therefore awaited.
      invoke('buildEnd', buildPluginContext(hookContext, pluginShareContext), [error])
    }
  }
  return normalized
}

function parseModuleInfo(moduleInfo) {
  if (typeof moduleInfo !== 'string') return moduleInfo
  try {
    return JSON.parse(moduleInfo)
  } catch {
    return {
      id: moduleInfo,
      code: null,
      ast: null,
      originalCode: null,
      importedIds: [],
      importers: [],
      isEntry: false,
      isExternal: false,
      isIncluded: null,
      hasDefaultExport: null,
      exports: null,
      dynamicallyImportedIds: [],
      exportedBindings: null,
      dynamicImporters: [],
    }
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
 * Merges the per-plugin share data seeded before Rolldown starts with any
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
function buildPluginContext(hookCtx, fallbackShare = {}) {
  const share = hookCtx.share || fallbackShare
  const moduleIds = hookCtx.moduleIds || []
  const moduleInfoMap = hookCtx.moduleInfoMap || {}
  const fileNames = hookCtx.fileNames || {}
  return {
    // hvigor share context
    share,
    // Hvigor forwards Rollup's IterableIterator-returning getModuleIds().
    getModuleIds() {
      return moduleIds.values()
    },
    // getModuleInfo(id) — returns { importedIds, dynamicallyImportedIds }
    getModuleInfo(id) {
      return moduleInfoMap[id] || null
    },
    // Hvigor forwards this to Rollup's Acorn-compatible parser.
    parse(source, options = {}) {
      return acorn.parse(source, {
        ecmaVersion: 'latest',
        sourceType: 'module',
        ...options,
      })
    },
    getFileName(referenceId) {
      if (Object.prototype.hasOwnProperty.call(fileNames, referenceId)) {
        return fileNames[referenceId]
      }
      const error = new Error(`Unable to get file name for unknown reference ID "${referenceId}"`)
      error.code = 'FILE_NAME_NOT_FOUND'
      throw error
    },
  }
}

function normalizeHookArguments(plugin) {
  const patches = {}
  let pluginShareContext = {}

  // The explicit API follows Rolldown/Rollup arguments. JSON is only the NAPI
  // transport; callers never observe that representation.
  if (typeof plugin.buildStart === 'function') {
    const original = plugin.buildStart
    patches.buildStart = function buildStart(optionsJson) {
      const inputOptions = parsePluginContext(optionsJson)
      return original.call(buildPluginContext({}, pluginShareContext), inputOptions)
    }
  }

  if (typeof plugin.beforeBuildEnd === 'function') {
    const original = plugin.beforeBuildEnd
    patches.beforeBuildEnd = function beforeBuildEnd(contextJson) {
      const hookCtx = parsePluginContext(contextJson)
      return original.call(buildPluginContext(hookCtx, pluginShareContext), hookCtx.input)
    }
  }

  if (typeof plugin.buildEnd === 'function') {
    const original = plugin.buildEnd
    patches.buildEnd = function buildEnd(contextJson) {
      const hookCtx = parsePluginContext(contextJson)
      const errors = Array.isArray(hookCtx.errors) ? hookCtx.errors : []
      let error
      if (errors.length > 0) {
        error = new Error(errors.map((item) => item.message || String(item)).join('\n'))
        error.name = 'RolldownError'
        error.errors = errors
      }
      return original.call(buildPluginContext(hookCtx, pluginShareContext), error)
    }
  }

  // NAPI does not supply a JavaScript receiver for Rolldown hook calls.
  // Hvigor wraps every compile hook with getCompilePluginContext, so bind the
  // same context here while retaining the original hook arguments.
  for (const hookName of ['resolveId', 'resolveDynamicImport', 'load', 'transform']) {
    if (typeof plugin[hookName] === 'function') {
      const original = plugin[hookName]
      patches[hookName] = function (...args) {
        return original.apply(buildPluginContext({}, pluginShareContext), args)
      }
    }
  }

  if (typeof plugin.moduleParsed === 'function') {
    const original = plugin.moduleParsed
    patches.moduleParsed = function (moduleInfo) {
      let parsedModuleInfo = moduleInfo
      if (typeof moduleInfo === 'string') {
        try {
          parsedModuleInfo = JSON.parse(moduleInfo)
        } catch {
          parsedModuleInfo = {
            id: moduleInfo,
            code: null,
            ast: null,
            originalCode: null,
            importedIds: [],
            importers: [],
            isEntry: false,
            isExternal: false,
            isIncluded: null,
            hasDefaultExport: null,
            exports: null,
            dynamicallyImportedIds: [],
            exportedBindings: null,
            dynamicImporters: [],
          }
        }
      }
      const moduleInfoMap = parsedModuleInfo && parsedModuleInfo.id ? { [parsedModuleInfo.id]: parsedModuleInfo } : {}
      return original.call(buildPluginContext({ moduleInfoMap }, pluginShareContext), parsedModuleInfo)
    }
  }

  if (typeof plugin.shouldInvalidCache === 'function') {
    const original = plugin.shouldInvalidCache
    patches.shouldInvalidCache = function shouldInvalidCache(request) {
      return original.call(buildPluginContext({}, pluginShareContext), parseShouldInvalidCacheRequest(request))
    }
  }

  const normalized = Object.keys(patches).length === 0 ? { ...plugin } : { ...plugin, ...patches }
  Object.defineProperty(normalized, '__arkdownSeedHvigorContext', {
    enumerable: false,
    value(contextJson) {
      const hookContext = parsePluginContext(contextJson)
      if (hookContext.share) pluginShareContext = hookContext.share
    },
  })
  return normalized
}

function parseShouldInvalidCacheRequest(request) {
  let parsed = request
  try {
    if (typeof request === 'string') parsed = JSON.parse(request)
  } catch {
    return { cacheHit: Boolean(request) }
  }
  if (parsed && typeof parsed === 'object' && parsed.ast === undefined && typeof parsed.code === 'string') {
    try {
      parsed.ast = acorn.parse(parsed.code, {
        ecmaVersion: 'latest',
        sourceType: 'module',
      })
    } catch {
      // Acorn cannot represent every lowered ArkTS/TypeScript extension. The
      // latest public type makes `ast` optional, so retain exact code/id and
      // omit the AST when parsing is not possible.
    }
  }
  return parsed
}

const ALL_PLUGIN_HOOKS = new Set([...ROLLUP_HOOKS, 'beforeBuildEnd', 'shouldInvalidCache'])

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
  const nodePathEntries = (process.env.NODE_PATH || '').split(path.delimiter).filter(Boolean)
  process.env.NODE_PATH = [nodeModules, ...nodePathEntries.filter((entry) => entry !== nodeModules)].join(
    path.delimiter,
  )
  Module._initPaths()
  installHvigorTypeScriptExtension()
  installHvigorResolvePatch()
  const priorContext = hvigorDependencyContexts.findIndex((context) => context.nodeModules === nodeModules)
  if (priorContext >= 0) {
    hvigorDependencyContexts.splice(priorContext, 1)
  }
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
  const dependencies =
    config && config.dependencies && typeof config.dependencies === 'object' ? { ...config.dependencies } : {}
  delete dependencies['@ohos/hvigor']
  delete dependencies['@ohos/hvigor-ohos-plugin']
  return { ...config, dependencies }
}

function installHvigorTypeScriptExtension() {
  if (hvigorTypeScriptExtensionInstalled) {
    return
  }
  const extension = function hvigorTypeScriptExtension(mod, filename) {
    const source = fs.readFileSync(filename, 'utf8')
    mod._compile(transpileHvigorSource(source, filename), filename)
  }
  // This is the exact extension pair installed by latest hvigor's
  // base/boot/hooks/require-hook.js.
  Module._extensions['.ts'] = extension
  Module._extensions['.mjs'] = extension
  hvigorTypeScriptExtensionInstalled = true
}

function transpileHvigorSource(source, filename) {
  let ts
  try {
    ts = require('typescript')
  } catch {
    throw new Error(`loading ${filename} requires the 'typescript' package`)
  }
  const transpiled = ts.transpileModule(source, {
    compilerOptions: {
      allowJs: true,
      alwaysStrict: false,
      declaration: false,
      esModuleInterop: false,
      forceConsistentCasingInFileNames: false,
      module: ts.ModuleKind.CommonJS,
      noEmit: false,
      noImplicitAny: false,
      noImplicitReturns: false,
      skipLibCheck: false,
      strict: false,
      target: ts.ScriptTarget.ES2016,
      types: [],
    },
    fileName: filename,
    reportDiagnostics: true,
  })
  if (transpiled.diagnostics?.length) {
    const message = ts.formatDiagnostics(transpiled.diagnostics, {
      getCanonicalFileName: (name) => name,
      getCurrentDirectory: () => process.cwd(),
      getNewLine: () => os.EOL,
    })
    const error = new SyntaxError(`failed to transpile ${filename}:${os.EOL}${message}`)
    error.code = 'HVIGOR_TYPESCRIPT_DIAGNOSTIC'
    throw error
  }
  return transpiled.outputText
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
  // Latest @ohos/hvigor 6.24.3 installs the same CommonJS TypeScript
  // extension handler for both .ts and .mjs. Do not use native ESM import:
  // it would bypass the hvigor dependency mocks and change module semantics.
  if (ext === '.ts' || ext === '.mjs') {
    return loadHvigorScriptModule(file, host)
  }
  // Clear the require cache so that reloads (watch mode, test isolation)
  // always get a fresh copy — matching hvigor's behaviour.
  const resolved = require.resolve(file)
  delete require.cache[resolved]
  return host ? host.loadWithModuleMocks(file, () => require(resolved)) : require(resolved)
}

// host: optional HvigorRuntimeHost for capturing task-factory config effects.
function loadHvigorScriptModule(file, host) {
  installHvigorTypeScriptExtension()
  const source = fs.readFileSync(file, 'utf8')
  const output = transpileHvigorSource(source, file)

  const mod = new Module(file, module.parent || module)
  mod.filename = file
  mod.paths = Module._nodeModulePaths(path.dirname(file)).concat(Module.globalPaths)
  if (host) {
    host.loadWithModuleMocks(file, () => mod._compile(output, file))
  } else {
    mod._compile(output, file)
  }
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

function mergeHvigorConfig(base, overlay) {
  if (overlay === undefined) {
    return base
  }
  if (
    base !== null &&
    overlay !== null &&
    typeof base === 'object' &&
    typeof overlay === 'object' &&
    !Array.isArray(base) &&
    !Array.isArray(overlay)
  ) {
    const merged = { ...base }
    for (const [key, value] of Object.entries(overlay)) {
      merged[key] = mergeHvigorConfig(merged[key], value)
    }
    return merged
  }
  return overlay
}

module.exports = {
  HvigorRuntimeHost,
  HvigorCompilePluginPlanner,
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
