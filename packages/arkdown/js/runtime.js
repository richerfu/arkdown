'use strict'

const fs = require('node:fs')
const crypto = require('node:crypto')
const Module = require('node:module')
const os = require('node:os')
const path = require('node:path')
const nativeRuntimeBinding = require('../index.js')

// Behavior baseline verified against the hvigor bundled with DevEco Studio
// 6.1.1. Keep this explicit so compatibility changes are evidence-driven.
const HVIGOR_PARITY_VERSION = '6.24.3'
const OHOS_COMMAND_BUILDER_TYPE = Object.freeze({
  0: 'NONE',
  1: 'CMAKE',
  2: 'NINJA',
  NONE: 0,
  CMAKE: 1,
  NINJA: 2,
})
const OHOS_PLUGIN_IDS = Object.freeze({
  app: 'com.ohos.app',
  hap: 'com.ohos.hap',
  har: 'com.ohos.har',
  hsp: 'com.ohos.hsp',
})
const OHOS_PLUGIN_ID_ENUM = Object.freeze({
  OHOS_APP_PLUGIN: OHOS_PLUGIN_IDS.app,
  OHOS_HAP_PLUGIN: OHOS_PLUGIN_IDS.hap,
  OHOS_HAR_PLUGIN: OHOS_PLUGIN_IDS.har,
  OHOS_HSP_PLUGIN: OHOS_PLUGIN_IDS.hsp,
})
const SIMPLE_OHOS_FACADE_FIELDS = {
  product: {
    getProductName: 'productName',
    getBundleType: 'bundleType',
    getBundleName: 'bundleName',
    getOutputArtifactName: 'outputArtifactName',
  },
  sdkDetails: {
    isOhos: 'isOhos',
    getSdkDir: 'sdkDir',
    getSdkVersion: 'sdkVersion',
    getEtsComponentVersion: 'etsComponentVersion',
    getEtsComponentReleaseType: 'etsComponentReleaseType',
  },
}
const UNSUPPORTED_HVIGOR_INTERNAL_EXPORTS = new Set(
  `
    AnalyzeModeMap CUSTOM_WORKER_REQUEST CacheStore CacheStoreManager ContinualEvent
    DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME DurationEvent ExtendedErrorInfo GlobalParam
    HVIGOR_PROJECT_WRAPPER_HOME HvigorBuildConst HvigorCommonConst HvigorConfigLoader HvigorLogger
    HvigorSystemPlugin HvigorTaskGroupType INCREMENTAL_OPTIMIZATION Json5Reader LocalFileWriter
    LogLevelMap MAIN_THREAD MarkEvent MarkEventType MetricEventType MetricFactory MetricLogType
    MetricService OHOS_ARK_COMPILE_SOURCE_MAP_DIR ParseJsonFile PathUtil PoolConstant
    ProjectCacheService ReplacePlaceHolderUtil ReportServiceImpl TaskDirectedAcyclicGraph TaskProxy
    Tasks TcbStore TraceManager TrackAPI WatchEvent WithParamReplacement WorkerEvent
    WorkerPoolDelegator addExtensionHandler addLayout configure connectLogger coreParameter
    doCompilePluginTypeCheck formatTime formatTimeToNumber getAlignTarget getConfiguration
    getHvigorConfigValue getLogger globalData hash hashFile iconv isConfigured isLinux isMac
    isWindows levelMap levels maxPathLength noop normalWorker parseJsonFile projectTaskDag
    readJavaDaemonInfoFromCacheFile recording replaceBundleName setCategoriesLevel shutdown
    splitTime startEnvironment watchWorker
  `
    .trim()
    .split(/\s+/),
)

const { createHvigorTaskUtilities } = require('./hvigor-task-runtime')
const { createHvigorParameterUtilities } = require('./hvigor-parameter-runtime')
const {
  createHvigorPublicFacade,
  createRuntimeLoggerFactory,
  createToolingModelRegistry,
} = require('./hvigor-public-runtime')
const { createHvigorWorkerRuntime } = require('./hvigor-worker-runtime')

function createHvigorPublicUtilities(host) {
  const publicFacade = createHvigorPublicFacade(nativeRuntimeBinding)
  const loggerFactory = createRuntimeLoggerFactory(nativeRuntimeBinding)
  const workerPool = createHvigorWorkerRuntime(nativeRuntimeBinding, host, {
    priority: publicFacade.Priority,
    worker: publicFacade.workerConstants,
  })
  const taskUtilities = createHvigorTaskUtilities(nativeRuntimeBinding, {
    getLogger: (category) => loggerFactory.getLogger(category),
    getWorkerPool: () => workerPool,
  })
  const defaultModelRegistry = createToolingModelRegistry(loggerFactory.getLogger('ToolingModelBeanRegistry'))
  const exports = {
    hvigor: {},
    BuildResult: publicFacade.BuildResult,
    FileUtil: publicFacade.FileUtil,
    Priority: publicFacade.Priority,
    TaskState: publicFacade.TaskState,
    replacer: publicFacade.replacer,
    ...taskUtilities,
    defaultModelRegistry,
  }
  const nativeFileApi = new nativeRuntimeBinding.RuntimeHvigorFileApi()
  const bindTaskToSession = (task, sync = false) => {
    wrapIncrementalDeclarations(task, exports)
    task.__arkdownClassTask = true
    task.__arkdownSyncTask = sync
    task.taskLog = loggerFactory.getLogger(task.getPath())
    return task
  }
  host._parameterUtilities.globalData.init(
    {
      ...process.env,
      cwd: host.projectRoot,
      configProps: new Map([['prop', new Map(Object.entries(host._parameter.getExtParams()))]]),
    },
    { ...host.config, _: host._commandEntryTasks },
  )
  Object.defineProperties(exports, {
    bindTaskToSession: { value: bindTaskToSession },
    hash: { value: (value) => nativeFileApi.hashText(String(value)) },
    loggerFactory: { value: loggerFactory },
    workerPool: { value: workerPool },
  })
  return exports
}

// Hook presence is projected once into Rust. The two latest-hvigor additions
// that Rolldown does not own remain scheduled by Rust gap adapters.
const PLUGIN_DISPATCH_HOOKS = [
  'buildStart',
  'resolveId',
  'load',
  'transform',
  'moduleParsed',
  'resolveDynamicImport',
  'beforeBuildEnd',
  'buildEnd',
  'shouldInvalidCache',
]
const ALL_PLUGIN_HOOKS = new Set(PLUGIN_DISPATCH_HOOKS)
const HVIGOR_DISCARDED_PLUGIN_RESULTS = new Set(['buildStart', 'moduleParsed', 'buildEnd'])
const PLUGIN_RESULT_FIELDS = {
  resolveId: 'resolveId',
  resolveDynamicImport: 'resolveId',
  load: 'load',
  transform: 'transform',
  shouldInvalidCache: 'shouldInvalidCache',
}
const PLUGIN_HOOK_ARGUMENTS = {
  resolveId: (request) => [request.id, request.importer, request.resolveOptions],
  resolveDynamicImport: (request) => [request.id, request.importer, request.resolveOptions],
  load: (request) => [request.id],
  transform: (request) => [request.code, request.id],
  shouldInvalidCache: (request) => [parseShouldInvalidCacheRequest(request.cacheRequest)],
  moduleParsed: (request) => [request.moduleInfo],
  beforeBuildEnd: () => [undefined],
}
const HVIGOR_LIFECYCLE_HOOKS = [
  'configEvaluated',
  'nodesInitialized',
  'beforeNodeEvaluate',
  'afterNodeEvaluate',
  'nodesEvaluated',
  'taskGraphResolved',
  'buildFinished',
  'onWatchWorkerMessage',
]

let hvigorTypeScriptExtensionInstalled = false

function createBuild(nativeBuild) {
  return (config = {}, plugins) => new HvigorCommandInvocation(nativeBuild, config, plugins, {}, false).run()
}

/**
 * Hvigor-compatible command entry point.
 *
 * The latest CLI stores raw command task names before evaluating
 * hvigorconfig/hvigorfile, resolves the complete lazy graph afterwards, and
 * only then starts task execution. Keep the same ordering here so plugins see
 * `hvigor.getCommandEntryTask()` during configuration and can still mutate the
 * selected tasks in `taskGraphResolved`.
 */
function createHvigorCommand(nativeBuild, nativeRuntime = {}) {
  return (config = {}, plugins) => new HvigorCommandInvocation(nativeBuild, config, plugins, nativeRuntime).run()
}

class HvigorCommandInvocation {
  constructor(nativeBuild, config, plugins, nativeRuntime = {}, commandMode = true) {
    this.nativeBuild = nativeBuild
    this.nativeRuntime = nativeRuntime
    this.runtime = new RuntimePluginLoader(config)
    this.explicitPlugins = plugins
    this.commandMode = commandMode
  }

  async run() {
    const startedAt = Date.now()
    let report
    let result
    let failure = null
    const host = this.runtime.hvigorHost
    const invocationToken = host._runtimeSession.beginInvocation()
    try {
      host.installObservability()
      const loadedPlugins = await this.runtime.load()
      const plugins = [...loadedPlugins, ...normalizePluginList(this.explicitPlugins, this.runtime.projectRoot)]
      await host.runTaskGraphResolved()
      host.typeCheckBuildScripts()
      if (this.commandMode) {
        report = await this.execute(host.commandPlan(), plugins)
        result = {
          artifacts: report.artifacts,
          elapsedMs: Date.now() - startedAt,
          output: report.output,
        }
        if (this.runtime.config.sync) await host.writeSyncInfo()
      } else {
        host.prepareDirectBuildCycle()
        try {
          result = await this.nativeBuild(this.runtime.toNativeBuildConfig(), plugins, host.createNativeTaskBridge())
        } finally {
          host.clearNativeTaskSelection()
        }
      }
      await host.writeBuildMeta()
      return result
    } catch (error) {
      failure = error
      throw error
    } finally {
      try {
        if (host.observabilityInstalled()) await host.runBuildFinished(failure, result || report)
        else await host._workerPool?.terminate()
      } finally {
        try {
          host.finishObservability()
        } finally {
          host._runtimeSession.finishInvocation(invocationToken)
        }
      }
    }
  }

  async execute(plan, plugins) {
    const host = this.runtime.hvigorHost
    const taskEntries = new Map(plan.tasks.map((entry) => [entry.taskPath, entry]))
    const materializedTasks = new Map()
    const materialize = (taskPath) => {
      if (!materializedTasks.has(taskPath)) materializedTasks.set(taskPath, host._commandTaskFromPath(taskPath))
      return materializedTasks.get(taskPath)
    }
    const customRoots = plan.tasks
      .filter((entry) => entry.executionLane === 'custom')
      .map((entry) => materialize(entry.taskPath))
    return host._runtimeSession.executeCommandPlan(plan, {
      executeStandalone: (taskPath) =>
        host.executeCommandTask(
          { ...taskEntries.get(taskPath), task: materialize(taskPath) },
          this.nativeRuntime.runRuntimeTasks,
        ),
      executeCustom: (taskPaths) =>
        host.executeCustomCommandTasks(taskPaths.map(materialize), this.nativeRuntime.runRuntimeTasks),
      executeNative: async (request) => {
        const nativeConfig = this.runtime.toNativeBuildConfig()
        nativeConfig.target = request.outputTarget ?? request.target
        nativeConfig.module = request.module
        nativeConfig.buildTargetName = request.targetName
        if (request.cutoffTaskRoots.length > 0) {
          nativeConfig.nativeTaskRootsJson = JSON.stringify(request.cutoffTaskRoots)
        }
        // DevEco Studio 6.1.1's cli.js accepts --watch, but 6.24.3 does not
        // route it into ordinary assemble task configuration. The command runs
        // once and exits; Arkdown's direct build({ watch: true }) API remains a
        // separate extension and still owns the persistent Rust watcher.
        nativeConfig.watch = false
        host.prepareNativeCommandCycle(request.nodeNames, customRoots, request.rootTaskPaths.map(materialize))
        let result
        try {
          result = await this.nativeBuild(nativeConfig, plugins, host.createNativeTaskBridge())
        } finally {
          host.clearNativeTaskSelection()
        }
        return toArray(result?.artifacts)
      },
    })
  }
}

class RuntimePluginLoader {
  constructor(config) {
    this.config = config || {}
    this.projectRoot = path.resolve(this.config.project || process.cwd())
    this.hvigorHost = new HvigorRuntimeHost(this.projectRoot, this.config)
    this.projectRoot = this.hvigorHost.projectRoot
    this.hvigorWorkspaceNodeModules = null
    this.hvigorWorkspacePrepared = false
    this.seen = new Set()
  }

  async load() {
    await this.prepareHvigorDependencyWorkspace()
    await this.hvigorHost.evaluateHvigorConfig()
    const plugins = [...(await this.loadConfiguredPluginPaths()), ...(await this.loadHvigorfiles())]
    await this.hvigorHost.prepareOhpmDependencyWorkspace()
    plugins.push(...(await this.loadBuildProfileCompilePlugins()))
    return new HvigorCompilePluginPlanner(this.config).apply(plugins)
  }

  async prepareHvigorDependencyWorkspace() {
    if (this.hvigorWorkspacePrepared) return
    const response = await this.hvigorHost._runtimeSession.prepareHvigorPluginWorkspace(
      os.homedir(),
      process.execPath,
      process.version,
    )
    if (response === null) {
      this.hvigorWorkspacePrepared = true
      return
    }
    if (response.error) {
      const { code, message, ...details } = response.error
      throw this.hvigorHost._error(code, message, details)
    }
    this.hvigorWorkspaceNodeModules = configureHvigorNodePath(
      this.projectRoot,
      response.workspace,
      response.nodeModules,
    )
    this.hvigorHost.hvigorWorkspaceNodeModules = this.hvigorWorkspaceNodeModules
    this.hvigorWorkspacePrepared = true
  }

  toNativeBuildConfig() {
    const startParams = this.hvigorHost.startParameters()
    this.hvigorHost._runtimePlan()
    const outcome = this.hvigorHost._runtimeSession.prepareNativeBuildConfig(
      this.config,
      {
        properties: this.hvigorHost._parameter.getPropertiesInternal(),
        extParams: this.hvigorHost._parameter.getExtParamsInternal(),
        coreStartParams: startParams,
        defaultCache: Boolean(startParams.incrementalExecution),
        mode: this.hvigorHost._effectiveBuildMode(),
      },
      process.execPath,
    )
    this.hvigorHost._throwBuildScriptDiagnostic(outcome.diagnostic)
    if (!outcome.config) {
      throw this.hvigorHost._error(
        'ARKDOWN_NATIVE_BUILD_CONFIG_MISSING',
        'Rust build-script runtime session returned no native build config',
      )
    }
    return outcome.config
  }

  async loadConfiguredPluginPaths() {
    const plugins = []
    for (const key of ['hvigorPluginFile', 'compilePluginPath']) {
      plugins.push(...(await this.loadPluginCandidates(this.config[key], this.projectRoot, { requireDefault: true })))
    }
    return plugins
  }

  async loadHvigorfiles() {
    const plugins = []
    const files = this.hvigorHost.discoverBuildScriptFiles(this.discoverHvigorRoots())
    const records = await this.hvigorHost.configureHvigorfiles(files)
    for (const record of records) {
      for (const plugin of await this.collectHvigorfileEffects(record)) {
        plugins.push(plugin)
      }
    }
    return plugins
  }

  discoverHvigorRoots() {
    return [this.projectRoot, ...this.hvigorHost.moduleRoots()]
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

    // Rust scans the stable hvigor option paths and applies extension
    // precedence. JavaScript only executes the selected plugin modules.
    const exportedPluginPaths = this.pluginPathPlan(value, baseDir)
    const loadOptions = { requireDefault: true, moduleRoot }
    plugins.push(...(await this.loadPluginCandidates(exportedPluginPaths.hvigorPluginFiles, baseDir, loadOptions)))
    plugins.push(...(await this.loadPluginCandidates(exportedPluginPaths.compilePluginFiles, baseDir, loadOptions)))
    // Programmatic task-factory and module-context mutations are collected
    // from the same merged object that is forwarded to Rust. The latest IDE
    // schemas reject a declarative `compilePluginFile`; that case is diagnosed
    // before this compatibility path is reached.
    const capturedPluginPaths = this.pluginPathPlan(capturedBuildOption, baseDir, false)
    plugins.push(...(await this.loadPluginCandidates(capturedPluginPaths.compilePluginFiles, baseDir, loadOptions)))

    // Collect plugin paths captured from task-factory calls (HvigorRuntimeHost effects).
    // This handles `hapTasks({ buildOption: { arkOptions: { compilePluginPath: '...' } } })`.
    plugins.push(...(await this.loadPluginCandidates(effects.hvigorPluginFile, baseDir, loadOptions)))
    plugins.push(...(await this.loadPluginCandidates(effects.compilePluginPath, baseDir, loadOptions)))

    return plugins
  }

  async loadBuildProfileCompilePlugins() {
    const plugins = []
    for (const record of this.hvigorHost.compilePluginRecords()) {
      plugins.push(
        ...(await this.loadPluginCandidates(record.file, record.moduleRoot, {
          requireDefault: true,
          moduleRoot: record.moduleRoot,
          targetName: record.targetName,
        })),
      )
    }
    return plugins
  }

  pluginPathPlan(value, baseDir, includeCompilePluginFile = true) {
    return this.hvigorHost._runtimeSession.collectPluginPaths(value, baseDir, includeCompilePluginFile)
  }

  captureHvigorOverrides(root, value, effects, file = path.join(root, 'hvigorfile.ts')) {
    const host = this.hvigorHost
    const ohos = value && typeof value === 'object' ? value.config?.ohos : undefined
    const overrides = ohos && typeof ohos === 'object' ? ohos.overrides : undefined
    const normalizedRoot = path.resolve(root)
    const response = host._runtimeSession.captureHvigorOverrides(normalizedRoot, overrides, effects.buildOption, file)
    host._throwBuildScriptDiagnostic(response.diagnostic)
    return response.buildOption
  }

  async loadPluginPath(candidate, baseDir = this.projectRoot, options = {}) {
    if (!candidate || typeof candidate !== 'string') return []
    const file = this.hvigorHost._runtimeSession.resolvePluginPath(candidate, baseDir)
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

  async loadPluginCandidates(candidates, baseDir, options) {
    const plugins = []
    for (const candidate of toArray(candidates))
      plugins.push(...(await this.loadPluginPath(candidate, baseDir, options)))
    return plugins
  }
}

// ---------------------------------------------------------------------------
// HvigorRuntimeHost
// ---------------------------------------------------------------------------

/**
 * Lazy built-in task catalog observed from DevEco Studio 6.1.1 / hvigor
 * 6.24.3. `LazyTaskContainer.findTask()` realizes one task on demand while
 * `getAllTasks()` returns only tasks that have already been realized.
 *
 * Rust owns the observed 6.24.3 creation-action dependencies and path
 * qualification. This class only preserves LazyTaskContainer's JavaScript API
 * for TypeScript plugin authors.
 */
function createHvigorTaskCatalog(host, node) {
  let definitions
  const load = () => {
    host._runtimePlan()
    const entries = host._runtimeSession.runtimeTaskCatalog(node.name, node.isProject)
    return new Map(entries.map((entry) => [entry.name, entry]))
  }
  return {
    invalidate: () => (definitions = undefined),
    get(name) {
      return (definitions ||= load()).get(name)
    },
    has(name) {
      return this.get(name) !== undefined
    },
    entries: () => (definitions ||= load()).entries(),
    request: () => {
      host._runtimePlan()
      return host._runtimeSession.runtimeTaskCatalogRequest(node.name, node.isProject)
    },
  }
}

function wrapIncrementalDeclarations(task, hvigor) {
  const state = {
    inputs: new Map(),
    inputFiles: new hvigor.FileSet(),
    outputFiles: new hvigor.FileSet(),
    ignored: new Set(),
  }
  const seen = new Set()
  let found = false
  const input = (name, value) => {
    const scalar = (entry) => ['number', 'string', 'boolean'].includes(typeof entry)
    if (scalar(value ?? '')) state.inputs.set(name, value ?? '')
    else if (Array.isArray(value) && value.every(scalar)) state.inputs.set(name, value)
    else {
      try {
        state.inputs.set(name, hvigor.hash(JSON.stringify(value)))
      } catch {}
    }
  }
  for (let owner = task; owner && owner !== Object.prototype; owner = Object.getPrototypeOf(owner)) {
    const records = Object.getOwnPropertySymbols(owner)
      .map((symbol) => owner[symbol])
      .find(
        (value) =>
          Array.isArray(value) &&
          value.every((entry) => entry && typeof entry === 'object' && 'declaratorType' in entry),
      )
    if (!records) continue
    found = true
    for (const record of records.filter((entry) => !state.ignored.has(entry.declaratorType))) {
      if (seen.has(record.propertyKey)) continue
      seen.add(record.propertyKey)
      const value = task[record.propertyKey]
      const options = record.options ? { ...record.options } : undefined
      if (record.declaratorType === 'Input') input(record.propertyKey, value)
      else if (record.declaratorType === 'Inputs' && value instanceof Map) {
        for (const [name, entry] of value) input(`${record.propertyKey}-${name}`, entry)
      } else if (record.declaratorType === 'InputFile') {
        if (typeof value === 'string' && value && fs.existsSync(value)) state.inputFiles.addEntry(value, options)
      } else if (record.declaratorType === 'InputFiles' && Array.isArray(value)) {
        for (const file of value)
          if (typeof file === 'string' && file && fs.existsSync(file)) state.inputFiles.addEntry(file, options)
      } else if (record.declaratorType === 'OutputFile') {
        if (typeof value === 'string' && value) state.outputFiles.addEntry(value, options)
      } else if (record.declaratorType === 'OutputFiles' && Array.isArray(value)) {
        for (const file of value) if (typeof file === 'string' && file) state.outputFiles.addEntry(file, options)
      } else if (record.declaratorType === 'IgnoreSuperIncremental') {
        const ignored = {
          declareInputs: ['Input', 'Inputs'],
          declareInputFiles: ['InputFile', 'InputFiles'],
          declareOutputFiles: ['OutputFile', 'OutputFiles'],
        }[record.propertyKey]
        for (const kind of ignored || []) state.ignored.add(kind)
      }
    }
  }
  if (!found) return
  if (state.inputs.size) {
    const original = task.declareInputs.bind(task)
    task.declareInputs = () => new Map([...original(), ...state.inputs])
  }
  for (const [method, additions] of [
    ['declareInputFiles', state.inputFiles],
    ['declareOutputFiles', state.outputFiles],
  ]) {
    if (!additions.collect().size) continue
    const original = task[method].bind(task)
    task[method] = () => {
      const files = original()
      for (const [file, options] of additions.collect()) files.addEntry(file, options ? { ...options } : undefined)
      return files
    }
  }
}

function aliasMethods(target, aliases) {
  for (const [alias, owner] of Object.entries(aliases)) target[alias] = target[owner]
  return target
}

function createRuntimeLifecycle(host) {
  const lifecycle = new nativeRuntimeBinding.RuntimeHvigorLifecycle()
  const invocationValues = new Map()
  let nextInvocation = 0
  return {
    register(name, callback) {
      if (typeof callback !== 'function') {
        throw host._error('INVALID_HVIGOR_LIFECYCLE_CALLBACK', `hvigor.${name} expects a function`)
      }
      lifecycle.register(name, (invocation) => callback(invocationValues.get(invocation)))
    },
    clear() {
      lifecycle.clear()
      invocationValues.clear()
    },
    async runHook(name, value) {
      const invocation = nextInvocation
      nextInvocation = (nextInvocation + 1) >>> 0
      invocationValues.set(invocation, value)
      try {
        await lifecycle.runHook(name, invocation)
      } finally {
        invocationValues.delete(invocation)
      }
    },
  }
}

function createNodeDescriptor(name, srcPath, parent, descriptor = {}) {
  const node = {
    name,
    srcPath,
    parent,
    targets: descriptor.targets,
    extraOptions: new Map(Object.entries(descriptor.extraOptions || {})),
    children: undefined,
    getChildNode() {
      return this.children
    },
    getRootNode() {
      return this.parent || this
    },
    setChildNode(children) {
      this.children = children
    },
  }
  return aliasMethods(node, { getChildNodeInternal: 'getChildNode', getRootNodeInternal: 'getRootNode' })
}

function createHvigorConfig(host) {
  const config = {
    root: createNodeDescriptor(path.basename(host.projectRoot), host.projectRoot),
    children: [],
    includeNode(name, srcPath, extraOptions) {
      const resolved = srcPath && !path.isAbsolute(srcPath) ? path.resolve(host.projectRoot, srcPath) : undefined
      if (!resolved || !fs.existsSync(resolved)) {
        const code = path.isAbsolute(srcPath || '') ? 'THE_SRCPATH_IS_NOT_A_RELATIVE_PATH' : 'SOME_PATH_NOT_FOUND'
        throw host._error(code, `module path does not exist: ${String(srcPath)}`)
      }
      this.children.push(createNodeDescriptor(name, srcPath, this.root, { extraOptions }))
      this.root.setChildNode(this.children)
    },
    excludeNodeByName(name) {
      this.children = this.children.filter((descriptor) => descriptor.name !== name)
      this.root.setChildNode(this.children)
    },
    getAllNodeDescriptor() {
      return this.children
    },
    getRootNodeDescriptor() {
      return this.root
    },
    getNodeDescriptorByName(name) {
      return this.children.find((descriptor) => descriptor.name === name)
    },
  }
  config.children = host._moduleDescriptors.map((descriptor) =>
    createNodeDescriptor(
      descriptor.name,
      descriptor.srcPath || path.relative(host.projectRoot, descriptor.root),
      config.root,
      descriptor,
    ),
  )
  config.root.setChildNode(config.children)
  return aliasMethods(config, {
    includeNodeInternal: 'includeNode',
    excludeNodeByNameInternal: 'excludeNodeByName',
    getAllNodeDescriptorInternal: 'getAllNodeDescriptor',
    getRootNodeDescriptorInternal: 'getRootNodeDescriptor',
    getNodeDescriptorByNameInternal: 'getNodeDescriptorByName',
  })
}

function createTaskContainer(core) {
  const container = {
    tasks: new Map(),
    lazyTasks: new Map(),
    registerTask(creation) {
      const name = creation?.details?.name
      if (!name || this.lazyTasks.has(name)) throw new Error(`Duplicate task ${String(name)}.`)
      this.lazyTasks.set(name, creation)
    },
    addTask(task) {
      this.tasks.set(task.getName(), task)
      return task
    },
    deleteTask(name) {
      if (!this.hasTask(name)) return true
      const deleted = this.tasks.delete(name)
      if (deleted) core.host._invalidateRuntimePlan()
      return deleted
    },
    hasTask(name) {
      return this.tasks.has(name) || this.lazyTasks.has(name)
    },
    getLazyTask(name) {
      return this.lazyTasks.get(name)
    },
    getAllTasks() {
      return [...this.tasks.values()]
    },
    getAllTasksCA() {
      return [...this.lazyTasks.values()]
    },
    getTaskPaths() {
      return [...new Set([...this.tasks.keys(), ...this.lazyTasks.keys()])]
    },
    findTask(name) {
      const creation = this.lazyTasks.get(name)
      if (!creation) return this.tasks.get(name)
      const task = this.tasks.get(name) || creation.provider()
      task.setDependsOn?.(...new Set([...toArray(task.getDependsOn?.()), ...toArray(creation.depends)]))
      this.tasks.set(name, task)
      return task
    },
    findTaskCA(name) {
      return this.lazyTasks.get(name)
    },
    clearTasks() {
      this.tasks.clear()
      this.lazyTasks.clear()
      core.host._invalidateRuntimePlan()
    },
  }
  return aliasMethods(container, { getTask: 'findTask' })
}

function createPublicNode(core, node) {
  const project = () => (node.isProject ? node : node.parent)
  const publicNode = {
    core,
    node,
    classKind: core.classKind,
    nodeDir: Object.freeze({ filePath: node.root, getPath: () => node.root }),
    getNodeName: () => core.getName(),
    getNodePath: () => node.root,
    getNodeDir() {
      return this.nodeDir
    },
    getParentNode: () => node.parent?.wrapper,
    getProject: () => project()?.wrapper,
    findModuleByName: (name) => project()?.children.find((child) => child.name === name)?.wrapper,
    getSubNodeByName: (name) => node.children.find((child) => child.name === name)?.wrapper,
    subNodes: (callback) => node.children.forEach((child) => callback(child.wrapper)),
    getContext: (pluginId) => core.getContext(pluginId),
    getAllPluginIds: () => core.getAllPluginIds(),
    getPluginById: (pluginId) => core.getPluginById(pluginId),
    registerTask: (task) => core.registerTask(task),
    getTaskByName: (name) => core.getTaskByName(name)?.getHvigorTask?.(),
    getAllTasks: () =>
      core
        .getAllTasks()
        .map((task) => task.getHvigorTask?.())
        .filter(Boolean),
    getConfigOpt: () => core.getConfigOpt(),
    addExtraOption: (key, value) => core.addExtraOption(key, value),
    getExtraOption: (key) => core.getExtraOption(key),
    beforeNodeEvaluate: (callback) => core.beforeNodeEvaluate(callback),
    afterNodeEvaluate: (callback) => core.afterNodeEvaluate(callback),
  }
  return aliasMethods(publicNode, {
    getName: 'getNodeName',
    getNodeNameInternal: 'getNodeName',
    getNodePathInternal: 'getNodePath',
  })
}

function createCoreNode(host, node) {
  const state = {
    plugins: new Map(),
    contexts: new Map(),
    pluginIds: [],
    extraOptions: new Map(),
    beforeNodeCallbacks: [],
    afterNodeCallbacks: [],
    config: undefined,
  }
  const project = () => (node.isProject ? node : node.parent)
  const core = {
    host,
    node,
    classKind: node.isProject ? 'project' : 'module',
    getName: () => node.name,
    getNodeDir: () => node.root,
    getBuildFilePath: () => host._normalizeFile(node.file),
    getBuildProfilePath: () => path.join(node.root, 'build-profile.json5'),
    getPackageJsonPath: () => path.join(node.root, host._usesOhpm ? 'oh-package.json5' : 'package.json'),
    getProject: () => project()?.coreWrapper,
    findModuleByName: (name) => project()?.children.find((child) => child.name === name)?.coreWrapper,
    addSubModule(childCore) {
      const child = childCore.node
      if (!node.children.includes(child)) node.children.push(child)
      child.parent = node
    },
    getAllSubModules: () => node.children.map((child) => child.coreWrapper),
    bindPlugin(plugin) {
      if (plugin?.getPluginId) state.plugins.set(plugin.getPluginId(), plugin)
      return plugin
    },
    bindPluginContextFunc(pluginId, context) {
      if (!state.pluginIds.includes(pluginId)) state.pluginIds.push(pluginId)
      state.contexts.set(pluginId, context)
    },
    getPluginById: (pluginId) => state.plugins.get(pluginId),
    getContext: (pluginId) => state.contexts.get(pluginId)?.(),
    getAllPluginIds: () => [...state.pluginIds],
    getTaskContainer: () => core.tasks,
    getAllTasks: () => core.tasks.getAllTasks(),
    getAllTasksCA: () => core.tasks.getAllTasksCA(),
    getTaskByName(name) {
      host._syncBuiltinCatalog(node)
      return core.tasks.findTask(name)
    },
    getTaskCAByName(name) {
      host._syncBuiltinCatalog(node)
      return core.tasks.findTaskCA(name)
    },
    getTaskPaths() {
      host._syncBuiltinCatalog(node)
      return core.tasks.getTaskPaths()
    },
    registry(task) {
      task.__arkdownUserTask ||= !task.__arkdownBuiltInDefinition
      const utilities = host._ensurePublicUtilities()
      utilities.bindTaskToSession(task, task instanceof utilities.DefaultSyncTask)
      core.tasks.addTask(task)
      host._invalidateRuntimePlan()
      return task
    },
    registerTask(definition) {
      return typeof definition?.getName === 'function' && typeof definition?.getAction === 'function'
        ? core.registry(definition)
        : host._registerDeclarativeTask(node, definition)
    },
    task(action, taskInfo) {
      const info = typeof taskInfo === 'string' || taskInfo === undefined ? { name: taskInfo || 'default' } : taskInfo
      const task = new (host._ensurePublicUtilities().DefaultTask)(node.wrapper, info)
      task.getAction = () => action || (() => {})
      return core.registry(task)
    },
    loadConfig(config) {
      state.config = config
    },
    getConfigOpt: () => ({
      getObject: (name) => state.config?.[name],
      setObject(name, value) {
        state.config ||= {}
        state.config[name] = value
      },
    }),
    addExtraOption: (key, value) => state.extraOptions.set(key, value),
    getExtraOption: (key) => state.extraOptions.get(key),
    beforeNodeEvaluate: (callback) => state.beforeNodeCallbacks.push(callback),
    afterNodeEvaluate: (callback) => state.afterNodeCallbacks.push(callback),
    async executeNodeHook(name) {
      const callbacks = name === 'beforeNodeEvaluate' ? state.beforeNodeCallbacks : state.afterNodeCallbacks
      while (callbacks.length) await callbacks.shift().call(core, node.wrapper)
    },
    async executeAfterBindSystemPluginsHook() {},
  }
  core.tasks = createTaskContainer(core)
  return core
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
    // CommonJS canonicalizes paths through filesystem aliases (for example
    // `/var` to `/private/var` on macOS) before exposing `__filename`. Keep
    // those loader-discovered spellings separate from the primary inventory
    // so node iteration remains unique and performs no additional realpath IO.
    this._nodeAliasesByFile = new Map()
    this._nodesByName = new Map()
    this._fallbackEffects = this._newEffects()
    this._effects = this._fallbackEffects
    this._activeFile = null
    this.hvigorWorkspaceNodeModules = null
    this._commandEntryTasks = toArray(this.config.tasks ?? this.config.task)
    try {
      this._runtimeSession = new nativeRuntimeBinding.RuntimeBuildScriptSession(
        this.projectRoot,
        getHvigorUserHome(),
        this.config,
        this._commandEntryTasks,
      )
    } catch (cause) {
      const marker = 'ARKDOWN_BUILD_SCRIPT_RUNTIME_DIAGNOSTIC:'
      const markerIndex = cause.message.indexOf(marker)
      if (markerIndex >= 0) {
        const diagnostic = JSON.parse(cause.message.slice(markerIndex + marker.length))
        const { code, message, ...details } = diagnostic
        throw this._error(code, message, details)
      }
      throw this._error(
        'INVALID_HVIGOR_MODEL_FILE',
        `failed to load the Rust build-script runtime session for ${this.projectRoot}: ${cause.message}`,
        { cause },
      )
    }
    const runtimeModel = this._runtimeSession.projectModel()
    this.projectRoot = path.resolve(runtimeModel.projectRoot)
    this._projectProfile = runtimeModel.projectProfile || {}
    this._projectHvigorConfig = runtimeModel.projectHvigorConfig || {}
    this._userHvigorConfig = runtimeModel.userHvigorConfig || {}
    this._buildConfigFile = runtimeModel.buildConfigFile
    this._fallbackModuleRoots = toArray(runtimeModel.fallbackModuleRoots).map((root) => path.resolve(root))
    this._selectedProductName = this.config.product || this._projectProfile.app?.products?.[0]?.name || 'default'
    this._usesOhpm = this._runtimeSession.usesOhpm
    this._projectPackage = this._usesOhpm ? runtimeModel.projectPackage || {} : runtimeModel.projectNpmPackage || {}
    this._moduleDescriptors = this._runtimeSession.moduleDescriptors()
    this._parameter = this._makeParameter()
    this._hvigorConfig = undefined
    this._hvigorCore = undefined
    this._hvigorApi = undefined
    this._hvigorExports = new Map()
    this._runtimeBuiltinActions = new Map()
    this._runtimeTaskFailures = new Map()
    this._hvigorTypeScriptRoots = new Set()
    this._toolingModelRegistry = undefined
    this._lifecycle = undefined
    this._processMonitor = undefined
    this._commandBuilderModifiers = new Map()
    this._syncNodeNames = undefined
    this._syncVersionChanged = this._readSyncVersionChanged()
  }

  startParameters() {
    return this._coreParameter.startParams
  }

  _ensurePublicUtilities() {
    if (!this._publicUtilities) {
      this._publicUtilities = createHvigorPublicUtilities(this)
      this._hvigorConfig = createHvigorConfig(this)
      this._workerPool = this._publicUtilities.workerPool
      this._toolingModelRegistry = this._publicUtilities.defaultModelRegistry
      this._toolingModelRegistry.clear()
      this._lifecycle = createRuntimeLifecycle(this)
    }
    return this._publicUtilities
  }

  installObservability() {
    this._ensurePublicUtilities()
    const consoleToken = String(process.env.ARKDOWN_LOG_CONSOLE || '')
      .trim()
      .toLowerCase()
    const consoleOwner = ['1', 'true', 'yes'].includes(consoleToken) ? 'tracing' : 'log4js'
    const logLevel = String(
      this.startParameters().logLevel?.levelStr || this.startParameters().logLevel || 'info',
    ).toLowerCase()
    const cacheDir = this.hvigorCacheDir()
    this._observabilityBuildId ||= this._parameterUtilities.globalData.buildId
    nativeRuntimeBinding.installRuntimeObservability({
      mode: 'napi',
      cacheDir,
      buildId: this._observabilityBuildId,
      logLevel,
      consoleOwner,
    })
    if (this.startParameters().analyze === 1) {
      this._processMonitor = new nativeRuntimeBinding.RuntimeProcessMonitor(() => process.memoryUsage(), process.pid)
      this._processMonitor.start()
    }
    this._observabilityInstalled = true
  }

  observabilityInstalled() {
    return this._observabilityInstalled === true
  }

  finishObservability() {
    if (!this._observabilityInstalled) return
    try {
      nativeRuntimeBinding.finishRuntimeObservability()
    } finally {
      this._observabilityInstalled = false
      this._observabilityEnabledCache = new Map()
    }
  }

  _isOhpmProject() {
    return this._usesOhpm
  }

  runtimeProperties(startParams) {
    return this.runtimePropertyPlan(startParams).runtimeProperties
  }

  hvigorCacheDir(logger) {
    const plan = this._runtimePathPlan()
    if (plan.hvigorCacheWarning && logger) logger.warn(plan.hvigorCacheWarning)
    return plan.hvigorCacheDir
  }

  async prepareOhpmDependencyWorkspace() {
    const response = await this._runtimeSession.prepareOhpmDependencyWorkspace({
      properties: this._parameter.getPropertiesInternal(),
      extParams: this._parameter.getExtParamsInternal(),
      executable: process.env.ohpmBin,
    })
    if (response.warning) this._warn(response.warning.code, response.warning.message)
    this._throwBuildScriptDiagnostic(response.error)
  }

  async writeSyncInfo() {
    this._ensurePublicUtilities()
    this._runtimePlan()
    const customModels = {}
    const customModelTaskInfo = {}
    for (const [modelId, bean] of this._toolingModelRegistry.getModelMap()) {
      if (typeof modelId !== 'string' || typeof bean?.buildModel !== 'function') {
        throw this._error(
          'INVALID_HVIGOR_TOOLING_MODEL',
          'ToolingModelBean must expose a string modelId and buildModel() method',
        )
      }
      customModels[modelId] = await bean.buildModel()
      customModelTaskInfo[modelId] = typeof bean.taskInfo === 'function' ? await bean.taskInfo() : []
    }

    const { projectCustomTasks, moduleCustomTasks } = this._runtimeTaskInventory(false)
    await this._runtimeSession.writeHvigorSyncModel(
      {
        configBuildVersion: this.config.buildVersion,
        selectedNodeNames: this._syncNodeNames ? Array.from(this._syncNodeNames) : undefined,
        projectCustomTasks,
        moduleCustomTasks,
        customModels,
        customModelTaskInfo,
      },
      this._parameter.getExtParamsInternal(),
    )
  }

  async writeBuildMeta() {
    await this._runtimeSession.writeHvigorBuildMeta(
      this._parameter.getPropertiesInternal(),
      this._parameter.getExtParamsInternal(),
    )
    this._syncVersionChanged = false
  }

  _readSyncVersionChanged() {
    return this._runtimeSession.hvigorSyncVersionChanged(
      this._parameter.getPropertiesInternal(),
      this._parameter.getExtParamsInternal(),
    )
  }

  _acceptContextMutation(outcome) {
    this._throwBuildScriptDiagnostic(outcome?.diagnostic)
    if (!outcome || !Object.prototype.hasOwnProperty.call(outcome, 'value')) {
      throw this._error('INVALID_HVIGOR_CONTEXT_MUTATION', 'Rust context session returned an invalid mutation')
    }
    return outcome
  }

  _contextPathOptions() {
    const plan = this._runtimePathPlan()
    return {
      buildCacheDir: plan.buildCacheDir,
      buildDir: plan.buildDir,
      previewMode: plan.previewMode,
      localTest: plan.localTest,
      ohosTest: plan.ohosTest,
    }
  }

  _runtimePathPlan() {
    return this._runtimeSession.runtimePathPlan(
      this._parameter.getPropertiesInternal(),
      this._parameter.getExtParamsInternal(),
    )
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
    const declared = this._moduleDescriptors.map((module) => module.root)
    return declared.length > 0 ? declared : [...this._fallbackModuleRoots]
  }

  async evaluateHvigorConfig() {
    this._ensurePublicUtilities()
    if (this._buildConfigFile) {
      this._activeFile = this._buildConfigFile
      if (this._buildConfigFile.endsWith('.ts')) this._hvigorTypeScriptRoots.add(this._buildConfigFile)
      await loadModule(this._buildConfigFile, this)
    }
    await this._lifecycle.runHook('configEvaluated', this._hvigorConfig)
    this._moduleDescriptors = this._runtimeSession.replaceModuleDescriptors(
      this._hvigorConfig.getAllNodeDescriptorInternal().map((descriptor) => ({
        name: descriptor.name,
        srcPath: descriptor.srcPath,
        root: path.resolve(this.projectRoot, descriptor.srcPath),
        targets: descriptor.targets,
        extraOptions: Object.fromEntries(descriptor.extraOptions),
      })),
    )
    this._invalidateRuntimePlan()
  }

  async runNodesInitialized() {
    await this._lifecycle.runHook('nodesInitialized', this._makeHvigorMock(this._activeFile).hvigor)
  }

  async runTaskGraphResolved() {
    await this._lifecycle.runHook('taskGraphResolved', this._makeHvigorMock(this._activeFile).hvigor)
    // User callbacks may mutate dependencies through official task objects
    // without going through registry(). Refresh the Rust snapshot once after
    // this last graph-mutation boundary.
    this._runtimeSession.invalidateRuntimeTaskModel()
  }

  commandPlan() {
    this._runtimePlan()
    const response = this._runtimeSession.runtimeCommandPlan({
      executionMode: this.config.executionMode,
      sync: Boolean(this.config.sync),
      taskNames: this._commandEntryTasks,
      taskModel: this._runtimeTaskInventory(false),
    })
    if (response.error) {
      const { code, message, ...details } = response.error
      throw this._error(code, message, details)
    }
    if (this.config.sync) {
      this._syncNodeNames = new Set(response.plan.selectedNodeNames)
    }
    for (const taskPath of new Set([
      ...response.plan.tasks.map((entry) => entry.taskPath),
      ...response.plan.nativeRequests.flatMap((request) => request.rootTaskPaths),
    ])) {
      this._commandTaskFromPath(taskPath)
    }
    this._runtimeSession.invalidateRuntimeTaskModel()
    return response.plan
  }

  _commandTaskFromPath(taskPath) {
    const separator = taskPath.indexOf(':')
    const nodeName = separator < 0 ? taskPath : taskPath.slice(0, separator)
    const taskName = separator < 0 ? taskPath : taskPath.slice(separator + 1)
    const node = nodeName
      ? this._nodesByName.get(nodeName)
      : Array.from(this._nodesByFile.values()).find((candidate) => candidate.isProject)
    const task = node ? this._getTaskByName(node, taskName) : undefined
    if (!task) {
      throw this._error(
        'ARKDOWN_RUNTIME_TASK_NOT_FOUND',
        `Rust command plan references unknown runtime task ${JSON.stringify(taskPath)}`,
        { taskPath },
      )
    }
    return task
  }

  async executeCommandTask(entry, runRuntimeTasks) {
    const { task, commandKind } = entry
    const action = async () => {
      const response = await this._runtimeSession.runStandaloneCommand({
        commandKind,
        nodeRoot: task.node.root,
        nodeName: task.node.name,
        projectTask: task.node.isProject,
        nodeKind: task.node.kind,
        tasks: this._runtimeTaskReportDescriptors(),
        module: this._parameter.getExtParam('module'),
        json: this._parameter.getExtParam('json') === '',
        resolve: this._parameter.getExtParam('buildOption') === '',
      })
      if (response.externalAction === 'collectCoverage') return this._generateCoverageReport()
      return this._maintenanceOutput(response, commandKind)
    }
    this._runtimeBuiltinActions.set(task.path, action)
    return this._executeRuntimeCommandTasks([task], runRuntimeTasks)
  }

  async pruneHvigorCaches(hvigorUserHome = getHvigorUserHome()) {
    return this._maintenanceOutput(await this._runtimeSession.runHvigorPrune(hvigorUserHome), 'prune')
  }

  async _generateCoverageReport(
    projectPath = this._parameter.getExtParam('projectPath'),
    reportPath = this._parameter.getExtParam('reportPath'),
    coverageFile = this._parameter.getExtParam('coverageFile'),
  ) {
    try {
      const pluginEntry = this.hvigorWorkspaceNodeModules
        ? require.resolve('@ohos/hvigor-ohos-plugin', { paths: [this.hvigorWorkspaceNodeModules] })
        : require.resolve('@ohos/hvigor-ohos-plugin')
      const pluginRequire = Module.createRequire(pluginEntry)
      const compose = pluginRequire('@ohos/hvigor-arkts-compose')
      if (typeof compose.generateReport !== 'function') {
        throw new TypeError('@ohos/hvigor-arkts-compose does not export generateReport')
      }
      await compose.generateReport(projectPath, reportPath, coverageFile)
    } catch (cause) {
      throw this._error(
        'ARKDOWN_COVERAGE_RUNTIME_NOT_FOUND',
        `collectCoverage requires the Hvigor 6.24.3 coverage runtime: ${cause.message}`,
        { cause },
      )
    }
    return ['Generate coverage statistics reports based on instrumentation dotting data']
  }

  _maintenanceOutput(response, operation) {
    if (!response || typeof response !== 'object') {
      throw this._error(
        'INVALID_ARKDOWN_MAINTENANCE_RESPONSE',
        `Rust ${operation} runner returned an invalid response`,
        { operation },
      )
    }
    if (response.error) {
      const { code, message, ...details } = response.error
      throw this._error(code, message, details)
    }
    if (!Array.isArray(response.output)) {
      throw this._error('INVALID_ARKDOWN_MAINTENANCE_RESPONSE', `Rust ${operation} runner returned no output array`, {
        operation,
      })
    }
    return response.output
  }

  async executeCustomCommandTasks(tasks, runRuntimeTasks) {
    if (tasks.length === 0) return
    await this._executeRuntimeCommandTasks(tasks, runRuntimeTasks)
  }

  async _executeRuntimeCommandTasks(tasks, runRuntimeTasks) {
    if (typeof runRuntimeTasks !== 'function') {
      throw this._error(
        'ARKDOWN_NATIVE_TASK_RUNTIME_UNAVAILABLE',
        'the Rust runtime task engine is required to execute hvigor command tasks',
      )
    }
    this._runtimeSession.prepareRuntimeTaskCycle(
      tasks.map((task) => task.path),
      tasks.map((task) => task.node.name),
    )
    try {
      return await runRuntimeTasks(this.createNativeTaskBridge({ validateBoundaries: false }))
    } finally {
      this.clearNativeTaskSelection()
    }
  }

  prepareNativeCommandCycle(nodeNames, customRoots, rootTasks) {
    this._runtimeSession.prepareCommandTaskCycle(
      nodeNames,
      customRoots.map((task) => task.path),
      rootTasks.map((task) => task.path),
    )
  }

  prepareDirectBuildCycle() {
    this._runtimePlan()
    for (const taskPath of this._runtimeSession.prepareDirectTaskCycle(this._runtimeTaskInventory(false))) {
      this._commandTaskFromPath(taskPath)
    }
    this._runtimeSession.invalidateRuntimeTaskModel()
    // Direct build() accepts third-party native executors that may only emit
    // boundaries carrying plugin work. Reachability still follows the full
    // assembly DAG, while command-mode partial tasks below require their
    // selected root boundary explicitly.
  }

  createNativeTaskBridge(options = {}) {
    const runtimeTasks = this._runtimeTaskInventory(false)
    const selection = this._runtimeSession.runtimeTaskSelection(options.validateBoundaries !== false)
    return {
      ...runtimeTasks,
      selection,
      dispatchTask: (request) => this._dispatchRuntimeTask(request),
      commandModifierTypes: Array.from(this._commandBuilderModifiers)
        .filter(([, modifier]) => modifier)
        .map(([builderType]) => builderType),
      hasAbcTransformConsumers: Array.from(this._nodesByFile.values()).some(
        (node) => node.transformAbcActions && node.transformAbcActions.size > 0,
      ),
      transformAbc: async (event) => {
        await this._runTransformAbcEvent(event)
      },
      modifyNativeCommand: (event) => this._runNativeCommandModifier(event),
    }
  }

  clearNativeTaskSelection() {
    this._runtimeSession.clearRuntimeTaskSelection()
    this._runtimeBuiltinActions.clear()
    this._runtimeTaskFailures.clear()
  }

  _runtimeTaskReportDescriptors() {
    return this._runtimeTaskInventory(true).tasks
  }

  _runtimeTaskInventory(includeCatalog) {
    const nodes = Array.from(this._nodesByFile.values())
    if (includeCatalog) for (const node of nodes) this._syncBuiltinCatalog(node)
    return this._runtimeSession.inspectRuntimeTaskModel(nodes, includeCatalog)
  }

  _runtimeTaskState(task) {
    return this._runtimeSession.inspectRuntimeTaskState(task.coreWrapper, task.taskContext, task.classIncrementalTask)
  }

  _runtimeTaskDeclarations(task) {
    return this._runtimeSession.inspectRuntimeTaskDeclarations(
      task.coreWrapper,
      task.taskContext,
      task.classIncrementalTask,
    )
  }

  async _dispatchRuntimeTask({ taskPath, phase, error: message } = {}) {
    const capture = async (action) => {
      try {
        const task = this._commandTaskFromPath(taskPath)
        if (!task)
          throw this._error('ARKDOWN_RUNTIME_TASK_NOT_FOUND', `Rust requested unknown runtime task ${taskPath}`)
        return await action(task)
      } catch (error) {
        this._runtimeTaskFailures.set(taskPath, error)
        throw error
      }
    }
    if (phase === 'failed') {
      const task = this._commandTaskFromPath(taskPath)
      const cause = this._runtimeTaskFailures.get(taskPath) || new Error(message || 'task failed')
      this._runtimeTaskFailures.delete(taskPath)
      task.taskContext.onFailed(cause)
      return {}
    }
    if (phase === 'realize') return capture((task) => this._runtimeTaskState(task))
    if (phase === 'beforeAlways') {
      return capture(async (task) => {
        await task.coreWrapper.beforeAlwaysAction()
        return this._runtimeTaskDeclarations(task)
      })
    }
    if (phase === 'taskShouldDo') {
      return capture((task) => ({ taskShouldDo: task.taskContext.taskShouldDo() !== false }))
    }
    if (phase === 'beforeRun') return capture(async (task) => (await this._runTaskBeforeHooks(task), {}))
    if (phase === 'afterRun') return capture(async (task) => (await this._runTaskAfterHooks(task), {}))
    if (phase === 'run') {
      return capture(async (task) => {
        if (task.builtIn) {
          const action = this._runtimeBuiltinActions.get(task.path)
          if (typeof action !== 'function') {
            throw this._error(
              'ARKDOWN_BUILTIN_TASK_ACTION_NOT_BOUND',
              `Rust requested an unbound standalone built-in task action for ${task.path}`,
            )
          }
          return { output: toArray(await action()) }
        }
        const action = task.coreWrapper.getAction()
        if (typeof action !== 'function') {
          throw this._error('INVALID_HVIGOR_TASK_RUN', `task ${task.key} must contain a run function`)
        }
        await action.call(task.coreWrapper)
        await this._awaitRuntimeTaskPendingPromises(task)
        return {}
      })
    }
    throw this._error(
      'ARKDOWN_RUNTIME_TASK_PHASE_NOT_FOUND',
      `Rust requested unknown runtime task phase ${String(phase)}`,
    )
  }

  async _awaitRuntimeTaskPendingPromises(task) {
    const pendingPromises = task.coreWrapper.pendingPromises || task.taskContext?.pendingPromises
    if (!pendingPromises || typeof pendingPromises.get !== 'function') return
    const pending = toArray(pendingPromises.get())
    pendingPromises.clear?.()
    await Promise.all(pending)
  }

  async runBuildFinished(error, _nativeReturn = null) {
    const utilities = this._ensurePublicUtilities()
    const report = nativeRuntimeBinding.runtimeHvigorReportSnapshot(process.ppid)
    const result = new utilities.BuildResult(error, report)
    try {
      await this._lifecycle.runHook('buildFinished', result)
    } finally {
      try {
        if (this.observabilityInstalled() && this.startParameters().analyze !== 2) {
          nativeRuntimeBinding.persistRuntimeReport(
            JSON.stringify(report, utilities.replacer),
            this._coreParameter.properties['hvigor.analyzeHtml'] === true,
          )
        }
      } finally {
        try {
          this._processMonitor?.stop()
        } finally {
          this._processMonitor = undefined
          await this._workerPool?.terminate()
        }
      }
    }
  }

  _loadModuleDescriptor(descriptor) {
    try {
      const model = this._runtimeSession.loadModuleDescriptor(descriptor)
      return model
    } catch (cause) {
      throw this._error('INVALID_HVIGOR_MODEL_FILE', `failed to load the Rust module runtime model: ${cause.message}`, {
        cause,
      })
    }
  }

  discoverBuildScriptFiles(roots) {
    try {
      return this._runtimeSession.discoverBuildScriptFiles(toArray(roots))
    } catch (cause) {
      throw this._error('INVALID_HVIGOR_MODEL_FILE', `failed to discover build scripts in Rust: ${cause.message}`, {
        cause,
      })
    }
  }

  _throwBuildScriptDiagnostic(diagnostic) {
    if (!diagnostic) return
    const { code, message, ...details } = diagnostic
    throw this._error(code, message, details)
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

  _makeParameter() {
    let plan
    try {
      plan = this._runtimeSession.parameterPlan()
    } catch (cause) {
      throw this._error(
        'ARKDOWN_HVIGOR_PARAMETER_PLAN_FAILED',
        `failed to materialize the Rust-owned Hvigor Parameter plan: ${cause.message}`,
        { cause },
      )
    }
    this._parameterUtilities = createHvigorParameterUtilities(nativeRuntimeBinding, plan)
    this._coreParameter = this._parameterUtilities.coreParameter
    return this._parameterUtilities.parameter
  }

  runtimePropertyPlan(startParams) {
    let outcome
    try {
      outcome = this._runtimeSession.runtimePropertyPlan(
        this._parameter.getPropertiesInternal(),
        this._parameter.getExtParamsInternal(),
        startParams || {},
      )
    } catch (cause) {
      throw this._error(
        'ARKDOWN_HVIGOR_RUNTIME_PROPERTY_PLAN_FAILED',
        `failed to resolve Rust-owned Hvigor runtime properties: ${cause.message}`,
        { cause },
      )
    }
    this._throwBuildScriptDiagnostic(outcome.diagnostic)
    if (!outcome.plan) {
      throw this._error('ARKDOWN_HVIGOR_RUNTIME_PROPERTY_PLAN_FAILED', 'Rust returned no Hvigor runtime property plan')
    }
    return outcome.plan
  }

  _effectiveBuildMode() {
    return this._parameter?.getExtParam('buildMode') || this.config.mode || this.config.buildMode || 'debug'
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
      if (file.endsWith('.ts')) this._hvigorTypeScriptRoots.add(file)
    }
    // BuildScriptFileInventory owns latest-hvigor root order: project first,
    // followed by the descriptor order selected by the Rust runtime model.
    // Hvigor always creates a root project model, even when no project-level
    // hvigorfile exists. Creating that core node first lets hvigorCore produce
    // the public HvigorNode wrappers instead of constructing its private
    // wrapper class ourselves.
    const projectFile =
      files.find((file) => path.dirname(path.resolve(file)) === this.projectRoot) ||
      path.join(this.projectRoot, 'hvigorfile.ts')
    this._nodeForFile(projectFile, true)
    for (const file of files) this._nodeForFile(file, true)
    const projectNode =
      this._nodesByFile.get(this._normalizeFile(path.join(this.projectRoot, 'hvigorfile.ts'))) ||
      Array.from(this._nodesByFile.values()).find((node) => node.root === this.projectRoot)
    if (projectNode) {
      for (const node of this._nodesByFile.values()) {
        if (node !== projectNode) {
          projectNode.coreWrapper.addSubModule(node.coreWrapper)
        }
      }
    }
  }

  effectsFor(file) {
    return this._nodeForFile(file, true).effects
  }

  async configureHvigorfiles(files) {
    if (files.length === 0) return []
    this.prepareHvigorfiles(files)
    await this.runNodesInitialized()
    return this._configureHvigorfiles(files)
  }

  async _configureHvigorfiles(files) {
    const records = []
    for (const file of files) {
      const node = this._nodeForFile(file, true)
      this._activeFile = node.file
      this._effects = node.effects
      await this._lifecycle.runHook('beforeNodeEvaluate', node.wrapper)
      await node.coreWrapper.executeNodeHook('beforeNodeEvaluate')
      const value = unwrapDefault(await loadModule(file, this))
      node.loaderConfig = value && typeof value === 'object' ? value.config : undefined
      node.exportValue = value
      node.coreWrapper.loadConfig(node.loaderConfig)
      const system = value?.system || value
      const systems = typeof system === 'function' ? [system] : Object.values(system || {})
      for (const candidate of systems) {
        const plugin = typeof candidate === 'function' ? await candidate(node.coreWrapper) : candidate
        if (!plugin?.getPluginId) continue
        node.coreWrapper.bindPlugin(plugin)
        node.coreWrapper.bindPluginContextFunc(plugin.getPluginId(), plugin.getContext.bind(plugin))
      }
      const plugins = toArray(value?.plugins).filter((plugin) => plugin && typeof plugin.apply === 'function')
      for (const plugin of plugins) {
        if (plugin.context) node.coreWrapper.bindPluginContextFunc(plugin.pluginId, plugin.context.bind(plugin))
      }
      for (const plugin of plugins) {
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
      await this._lifecycle.runHook('afterNodeEvaluate', node.wrapper)
      await node.coreWrapper.executeNodeHook('afterNodeEvaluate')
      records.push({ file, value })
    }
    await this._lifecycle.runHook('nodesEvaluated', this._makeHvigorMock(this._activeFile).hvigor)
    return records
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
    let node = this._nodesByFile.get(normalized) || this._nodeAliasesByFile.get(normalized)
    if (node || !create) {
      return node
    }
    const actualFile = path.resolve(file)
    const root = path.dirname(actualFile)
    const isProject = root === this.projectRoot
    const descriptor = isProject
      ? undefined
      : this._runtimeSession.moduleDescriptor(root) ||
        this._loadModuleDescriptor({
          name: path.basename(root),
          srcPath: path.relative(this.projectRoot, root),
          root,
        })
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
      catalogTaskNames: new Set(),
      transformAbcActions: new Map(),
      loaderConfig: undefined,
      exportValue: undefined,
      moduleProfile: isProject ? this._projectProfile : descriptor.moduleProfile || {},
      moduleJson: isProject ? undefined : descriptor.moduleJson,
      modulePackage: isProject ? this._projectPackage : descriptor.modulePackage || {},
    }
    node.taskCatalog = createHvigorTaskCatalog(this, node)
    this._ensurePublicUtilities()
    node.coreWrapper = createCoreNode(this, node)
    node.wrapper = createPublicNode(node.coreWrapper, node)
    this._runtimeSession.upsertModule(node)
    for (const [key, value] of Object.entries(descriptor?.extraOptions || {}))
      node.coreWrapper.addExtraOption(key, value)
    this._initializeCoreTasks(node)
    this._nodesByFile.set(normalized, node)
    this._nodesByName.set(name, node)
    this._invalidateRuntimePlan()
    return node
  }

  _aliasBuildScriptFile(alias, source) {
    const node = this._nodeForFile(source, false)
    if (node) {
      this._nodeAliasesByFile.set(this._normalizeFile(alias), node)
    }
  }

  _pluginIdForKind(kind) {
    return OHOS_PLUGIN_IDS[kind]
  }

  _invalidateRuntimePlan() {
    this._runtimeSession.invalidate()
  }

  _runtimePlan() {
    const rawModuleSelection = this._parameter.getExtParam('module') ?? this.config.module
    const response = this._runtimeSession.plan({
      productName: this._selectedProductName,
      buildModeName: this._effectiveBuildMode(),
      moduleSelection: typeof rawModuleSelection === 'string' ? rawModuleSelection : undefined,
      properties: this._coreParameter.properties,
      extParams: this._coreParameter.extParams,
      daemon: Boolean(this.startParameters().daemon),
      syncVersionChanged: this._syncVersionChanged,
    })
    if (response.diagnostic) {
      const { code, message, ...details } = response.diagnostic
      throw this._error(code, message, details)
    }
    return response.prepared
  }

  compilePluginRecords() {
    this._runtimePlan()
    return this._runtimeSession.compilePluginRecords()
  }

  _readOhosContext(node, operation, argument) {
    const value = this._runtimeSession.readOhosContext({
      operation,
      root: node.isProject ? undefined : node.root,
      argument,
      buildMode: this._effectiveBuildMode(),
      debugLine: String(this._parameter.getExtParam('ohos-debug-line')) === 'true',
      pathOptions: this._contextPathOptions(),
    })
    return this._materializeOhosValue(value)
  }

  _writeOhosContext(node, operation, value) {
    const outcome = this._acceptContextMutation(
      this._runtimeSession.writeOhosContext({
        operation,
        root: node.isProject ? undefined : node.root,
        value,
      }),
    )
    if (outcome.selectedProductName) this._selectedProductName = outcome.selectedProductName
    if (operation === 'setBuildProfileOpt') {
      node.taskCatalog.invalidate()
      node.parent?.taskCatalog.invalidate()
    }
  }

  _materializeOhosValue(value) {
    if (!value || typeof value !== 'object') return value === null ? undefined : value
    if (Array.isArray(value)) return value.map((entry) => this._materializeOhosValue(entry))
    const fields = SIMPLE_OHOS_FACADE_FIELDS[value.__arkdownFacade]
    if (fields) return Object.fromEntries(Object.entries(fields).map(([method, field]) => [method, () => value[field]]))
    if (value.__arkdownFacade === 'target') {
      return {
        getTargetName: () => value.targetName,
        getCurrentProduct: () => this._materializeOhosValue(value.product),
        getBuildOption: () => this._deepFreeze(structuredClone(value.buildOption)),
        getBuildTargetOutputPath: () => value.paths.buildTargetOutputPath,
        getModulePathDetails: () => ({
          getModuleBuildPath: () => value.paths.moduleBuildPath,
          getIntermediatesRes: () => value.paths.intermediatesResPath,
          getModuleBuildCachePath: () => value.paths.moduleBuildCachePath,
        }),
      }
    }
    return value
  }

  _ohosContextFor(node) {
    if (node.ohosContext) return node.ohosContext
    if (!node.isProject) this._runtimePlan()
    const context = {}
    const { reads, writes, packageMethods } = this._runtimeSession.ohosContextMethods(node.isProject)
    for (const name of reads) context[name] = (argument) => this._readOhosContext(node, name, argument)
    for (const [getter, setter] of packageMethods) {
      context[getter] = () => this._readOhosContext(node, getter)
      context[setter] = (value) => this._writeOhosContext(node, setter, value)
    }
    for (const name of writes) context[name] = (value) => this._writeOhosContext(node, name, value)
    if (!node.isProject) {
      context.targets = (callback) => this._readOhosContext(node, 'targets').forEach(callback)
      context.getTargetArtifactNames = () => new Map(this._readOhosContext(node, 'getTargetArtifactNames'))
      context.loadCompilePlugin = (pluginPath) => this._loadCompilePlugin(node, pluginPath)
      context.transformAbc = (callback, targetName) => node.transformAbcActions.set(targetName ?? 'all', callback)
    }
    node.ohosContext = context
    return context
  }

  _typeCheckCompilePlugin(pluginFile) {
    this._ensurePublicUtilities()
    try {
      const ts = require('typescript')
      const program = ts.createProgram([pluginFile], {
        baseUrl: this.hvigorWorkspaceNodeModules || path.join(this.projectRoot, 'node_modules'),
        target: ts.ScriptTarget.ES2016,
        module: ts.ModuleKind.CommonJS,
        types: [],
        allowJs: true,
        esModuleInterop: true,
        forceConsistentCasingInFileNames: true,
        strict: true,
        skipLibCheck: true,
        noImplicitAny: false,
        noImplicitReturns: false,
        alwaysStrict: false,
        declaration: false,
        noEmit: true,
      })
      const diagnostics = ts.getPreEmitDiagnostics(program)
      if (diagnostics.length) {
        const formatHost = {
          getCanonicalFileName: (value) => value,
          getCurrentDirectory: ts.sys.getCurrentDirectory,
          getNewLine: () => ts.sys.newLine,
        }
        const cause = new Error(ts.formatDiagnostics(diagnostics, formatHost))
        cause.name = 'AdaptorError'
        throw cause
      }
    } catch (cause) {
      throw this._error('TYPE_ERRORS', cause.message, { cause, pluginPath: pluginFile })
    }
  }

  typeCheckBuildScripts() {
    if (!this.startParameters().hvigorfileTypeCheck || this._hvigorTypeScriptRoots.size === 0) return
    const ts = require('typescript')
    const roots = [...this._hvigorTypeScriptRoots].filter((file) => fs.existsSync(file))
    const program = ts.createProgram(roots, {
      baseUrl: this.hvigorWorkspaceNodeModules || path.join(this.projectRoot, 'node_modules'),
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
    })
    const diagnostics = ts.getPreEmitDiagnostics(program)
    try {
      if (diagnostics.length) throw new Error(`TypeScript errors in ${roots.join(', ')}`)
    } catch (cause) {
      throw this._error('TYPE_ERRORS', cause.message, { cause, files: [...this._hvigorTypeScriptRoots] })
    }
  }

  _loadCompilePlugin(node, pluginPath) {
    const outcome = this._runtimeSession.resolveCompilePluginPath(node.root, pluginPath)
    this._throwBuildScriptDiagnostic(outcome.diagnostic)
    const resolved = outcome.path
    // Latest OhosModuleContextImpl calls doCompilePluginTypeCheck before
    // storing the plugin path. Keep the same TypeScript 4.9.5 program options
    // here; execution itself remains in Arkdown's Rolldown driver.
    this._typeCheckCompilePlugin(resolved)
    node.effects.compilePluginPath.push(resolved)
  }

  _registerDeclarativeTask(node, definition) {
    if (!definition || typeof definition !== 'object' || typeof definition.name !== 'string') {
      throw this._error('INVALID_HVIGOR_TASK', `custom task on ${node.name} must contain a string name`)
    }
    if (node.coreWrapper.getTaskContainer().hasTask(definition.name)) {
      throw this._error('DUPLICATE_HVIGOR_TASK', `duplicate task ${node.name}:${definition.name}`, {
        taskName: definition.name,
        moduleName: node.name,
      })
    }
    const utilities = this._ensurePublicUtilities()
    const task = new utilities.IncrementalExecTask(node.wrapper, {
      name: definition.name,
      group: 'Customize',
      sourcePluginId: 'custom-plugin',
    })
    const inputs = new Map()
    const inputFiles = new utilities.FileSet()
    const outputFiles = new utilities.FileSet()
    const taskContext = {
      ...definition,
      context: definition.context == null ? undefined : definition.context.apply(),
    }
    const taskRunContext = { moduleName: node.name, modulePath: node.root }
    const taskInput = {
      file: (file, options) => (inputFiles.addEntry(file, options), taskInput),
      files: (files, options) => (toArray(files).forEach((file) => inputFiles.addEntry(file, options)), taskInput),
      property: (name, value) => (inputs.set(name, value), taskInput),
    }
    const taskOutput = {
      file: (file, options) => (outputFiles.addEntry(file, options), taskOutput),
      files: (files, options) => (toArray(files).forEach((file) => outputFiles.addEntry(file, options)), taskOutput),
    }
    definition.input?.apply(taskContext, [taskInput])
    definition.output?.apply(taskContext, [taskOutput])
    task.task = definition
    task.taskContext = taskContext
    task.declareInputs = () => inputs
    task.declareInputFiles = () => inputFiles
    task.declareOutputFiles = () => outputFiles
    task.setDependsOn(...toArray(definition.dependencies))
    if (typeof definition.beforeRun === 'function') {
      task.beforeRun(() => definition.beforeRun.apply(taskContext, [taskRunContext]))
    }
    if (typeof definition.afterRun === 'function') {
      task.afterRun(() => definition.afterRun.apply(taskContext, [taskRunContext]))
    }
    task.getAction = () => async () => {
      if (typeof definition.run === 'function') await definition.run.apply(taskContext, [taskRunContext])
    }
    task.__arkdownUserTask = true
    node.coreWrapper.registry(task)
    for (const reference of toArray(definition.postDependencies)) {
      const target = this._taskReference(node, reference)
      const creation = target.node?.coreWrapper.getTaskCAByName(target.taskName)
      if (creation) creation.dependsDefinedByUser.push(task.getPath())
      else target.node?.coreWrapper.getTaskByName(target.taskName)?.addDependsOn(task.getPath())
    }
    return definition
  }

  _taskReference(owner, reference) {
    if (typeof reference !== 'string') return { node: owner, taskName: String(reference) }
    const separator = reference.indexOf(':')
    if (separator < 0) return { node: owner, taskName: reference }
    const nodeName = reference.slice(0, separator)
    const node = nodeName
      ? this._nodesByName.get(nodeName)
      : [...this._nodesByFile.values()].find((item) => item.isProject)
    return { node, taskName: reference.slice(separator + 1) }
  }

  _initializeCoreTasks(node) {
    const container = node.coreWrapper.getTaskContainer()
    for (const name of node.isProject ? ['tasks', 'taskTree', 'prune'] : ['tasks', 'taskTree']) {
      if (container.hasTask(name)) continue
      const definition = { name, dependencies: [], group: 'Help', description: '' }
      const coreTask = new (this._ensurePublicUtilities().DefaultTask)(node.coreWrapper, definition)
      coreTask.__arkdownBuiltInDefinition = definition
      container.addTask(coreTask)
    }
  }

  _getTaskByName(node, name) {
    this._syncBuiltinCatalog(node)
    const coreTask = node.coreWrapper.getTaskByName(name)
    if (!coreTask) return undefined
    return this._runtimeTask(node, coreTask)
  }

  _syncBuiltinCatalog(node) {
    const container = node.coreWrapper.getTaskContainer()
    for (const [name, definition] of node.taskCatalog.entries()) {
      const existing = container.getLazyTask(name)
      if (existing?.__arkdownBuiltInDefinition) {
        existing.depends = [...definition.dependencies]
        existing.details = {
          name,
          group: definition.group,
          description: definition.description,
          sourcePluginId: definition.sourcePluginId,
        }
        existing.__arkdownBuiltInDefinition = definition
        continue
      }
      if (existing) continue
      const creation = {
        provider: () => {
          const task = new (this._ensurePublicUtilities().DefaultTask)(node.coreWrapper, {
            name,
            group: definition.group,
            description: definition.description,
            sourcePluginId: definition.sourcePluginId,
          })
          task.__arkdownBuiltInDefinition = creation.__arkdownBuiltInDefinition
          return task
        },
        depends: [...definition.dependencies],
        details: {
          name,
          group: definition.group,
          description: definition.description,
          sourcePluginId: definition.sourcePluginId,
        },
        dependsDefinedByUser: [],
        __arkdownBuiltInDefinition: definition,
      }
      container.registerTask(creation)
      node.catalogTaskNames.add(name)
    }
  }

  _runtimeTask(node, coreTask) {
    const classIncrementalTask =
      typeof coreTask.declareInputs === 'function' &&
      typeof coreTask.declareInputFiles === 'function' &&
      typeof coreTask.declareOutputFiles === 'function'
    const taskContext = coreTask.taskContext || coreTask
    return {
      key: `${node.name}:${coreTask.getName()}`,
      path: coreTask.getPath(),
      node,
      taskContext,
      builtIn: Boolean(coreTask.__arkdownBuiltInDefinition),
      classIncrementalTask,
      coreWrapper: coreTask,
      get dependencies() {
        return coreTask.getDependsOn()
      },
    }
  }

  async _runTaskBeforeHooks(task) {
    await task.coreWrapper.executeBeforeRun()
  }

  async _runTaskAfterHooks(task) {
    await task.coreWrapper.executeAfterRun()
  }

  async _runTransformAbcEvent(event) {
    if (
      typeof event.moduleName !== 'string' ||
      typeof event.targetName !== 'string' ||
      typeof event.abcPath !== 'string' ||
      !event.config ||
      typeof event.config.isArkGuardEnabled !== 'boolean'
    ) {
      throw this._error('INVALID_ARKDOWN_TRANSFORM_ABC_EVENT', `invalid transformAbc event: ${JSON.stringify(event)}`)
    }
    const node = this._nodesByName.get(event.moduleName)
    if (!node) {
      throw this._error(
        'ARKDOWN_TRANSFORM_ABC_NODE_NOT_FOUND',
        `transformAbc references unknown node ${JSON.stringify(event.moduleName)}`,
      )
    }
    const callback = node.transformAbcActions.get(event.targetName) ?? node.transformAbcActions.get('all')
    if (callback === undefined) {
      return true
    }
    await Reflect.apply(callback, undefined, [event.abcPath, this._clone(event.config)])
    return true
  }

  _runNativeCommandModifier(event) {
    const modifier = this._commandBuilderModifiers.get(event?.builderType)
    if (!modifier) return event.command
    const command = modifier.modify([...event.command])
    if (!Array.isArray(command) || command.some((argument) => typeof argument !== 'string')) {
      throw this._error(
        'ARKDOWN_INVALID_COMMAND_MODIFIER_RESULT',
        `CommandBuilderModifier.modify() must return string[] for builder type ${String(event.builderType)}`,
        { builderType: event.builderType },
      )
    }
    return command
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
    return this._systemFactory(kind || 'hap', file)
  }

  _systemFactory(kind, file) {
    const factory = (value) => {
      if (typeof value?.getBuildFilePath !== 'function') {
        return this._captureTaskEffect(value, file, kind)
      }
      const node = this._nodesByName.get(value.getName()) || this._nodeForFile(value.getBuildFilePath(), true)
      node.kind = kind
      this._runtimeSession.upsertModule(node)
      this._invalidateRuntimePlan()
      node.taskCatalog.invalidate()
      node.parent?.taskCatalog.invalidate()
      const pluginId = this._pluginIdForKind(kind)
      const plugin = {
        getPluginId: () => pluginId,
        getContext: () => this._ohosContextFor(node),
      }
      value.bindPluginContextFunc(pluginId, plugin.getContext)
      return plugin
    }
    Object.defineProperty(factory, '__arkdownHvigorSystemKind', { value: kind })
    return factory
  }

  _legacySystemFactory(kind, file) {
    const factory = () => {
      throw this._error(
        'ARKDOWN_UNSUPPORTED_LEGACY_FA_MODEL',
        `legacy${kind[0].toUpperCase()}${kind.slice(1)}Tasks requires the legacy FA/JSBUNDLE pipeline, which arkdown does not support`,
        { kind, file },
      )
    }
    Object.defineProperty(factory, '__arkdownHvigorSystemKind', { value: `legacy-${kind}` })
    return factory
  }

  _publicNodePath(node) {
    if (typeof node?.getNodePath === 'function') return path.resolve(node.getNodePath())
    const nodeDir = node?.getNodeDir?.()
    if (typeof nodeDir === 'string') return path.resolve(nodeDir)
    if (typeof nodeDir?.getPath === 'function') return path.resolve(nodeDir.getPath())
    throw this._error('FAILED_TO_GET_REMOTE_HSP_NODE_PATH', 'getRemoteHspObjList expects an Hvigor node')
  }

  _getRemoteHspObjList(node) {
    const moduleRoot = this._publicNodePath(node)
    const project = node?.getProject?.()
    const projectRoot = project ? this._publicNodePath(project) : this.projectRoot
    const response = this._runtimeSession.inspectRemoteHspObjects(moduleRoot, projectRoot)
    if (response.error) {
      const { code, message, ...details } = response.error
      throw this._error(code, message, details)
    }
    return response.objects
  }

  _registryCommandModifier(builderType, modifier) {
    this._commandBuilderModifiers.set(builderType, modifier)
  }

  _submitWorker(publicNode, taskName, workPath, options) {
    const nodeName = publicNode?.getNodeNameInternal?.() ?? publicNode?.getNodeName?.()
    const node = this._nodesByName.get(nodeName)
    if (!node) {
      throw this._error('FAILED_TO_SUBMIT_WORKER_MODULE_NOT_FOUND', `worker node ${String(nodeName)} was not found`)
    }
    const task = this._getTaskByName(node, taskName)
    if (!task) {
      throw this._error(
        'FAILED_TO_SUBMIT_WORKER_TASK_NOT_FOUND',
        `worker task ${String(taskName)} was not found on node ${node.name}`,
      )
    }
    if (typeof workPath !== 'string') {
      throw new TypeError('submitWorker workPath must be a string')
    }
    return this._workerPool.submit(task.coreWrapper, workPath, options)
  }

  _makeOhosPluginMock(file = this._activeFile) {
    return {
      OhosPluginId: OHOS_PLUGIN_ID_ENUM,
      appTasks: this._systemFactory('app', file),
      hapTasks: this._systemFactory('hap', file),
      hspTasks: this._systemFactory('hsp', file),
      harTasks: this._systemFactory('har', file),
      legacyAppTasks: this._legacySystemFactory('app', file),
      legacyHapTasks: this._legacySystemFactory('hap', file),
      legacyHarTasks: this._legacySystemFactory('har', file),
      getRemoteHspObjList: (node) => this._getRemoteHspObjList(node),
      CommandBuilderType: OHOS_COMMAND_BUILDER_TYPE,
      registryCommandModifier: (builderType, modifier) => this._registryCommandModifier(builderType, modifier),
    }
  }

  _makeHvigorMock(file = this._activeFile) {
    const host = this
    const callerFile = file || host._activeFile
    const cacheKey =
      typeof callerFile === 'string' && callerFile.length > 0 ? host._normalizeFile(callerFile) : '<unbound>'
    const cached = host._hvigorExports.get(cacheKey)
    if (cached) return cached
    const publicUtilities = host._ensurePublicUtilities()
    const rootNode = () => Array.from(host._nodesByFile.values()).find((node) => node.isProject)?.wrapper
    const allNodes = () => {
      const nodes = Array.from(host._nodesByFile.values())
      const project = nodes.find((node) => node.isProject)
      return [...nodes.filter((node) => node !== project), ...(project ? [project] : [])].map((node) => node.wrapper)
    }
    const lifecycle = (name) => (callback) => host._lifecycle.register(name, callback)
    const lifecycleApi = Object.fromEntries(HVIGOR_LIFECYCLE_HOOKS.map((name) => [name, lifecycle(name)]))
    const hvigor = (host._hvigorApi ||= Object.assign(Object.create(publicUtilities.hvigor), {
      getRootNode: rootNode,
      getAllNodes: allNodes,
      getNodeByName: (name, classKind) =>
        allNodes().find(
          (node) => node.getNodeName() === name && (classKind === undefined || node.classKind === classKind),
        ),
      getHvigorConfig: () => host._hvigorConfig,
      getParameter: () => host._parameter,
      ...lifecycleApi,
      getCommandEntryTask: () => (host._commandEntryTasks.length ? [...host._commandEntryTasks] : undefined),
      isCommandEntryTask: (taskName) => host._commandEntryTasks.includes(taskName),
      getExtraConfig: () => new Map(Object.entries(host._parameter.getExtParamsInternal())),
      getProject: rootNode,
      getHvigorUserHome,
      getHvigorVersion: () => HVIGOR_PARITY_VERSION,
    }))
    const hvigorCore = (host._hvigorCore ||= {
      getHvigorVersion: () => HVIGOR_PARITY_VERSION,
      getProject: () => [...host._nodesByFile.values()].find((node) => node.isProject)?.coreWrapper,
      getParameter: () => host._parameter,
      getExtraConfig: () => hvigor.getExtraConfig(),
      getCommandEntryTasks: () => hvigor.getCommandEntryTask(),
      isCommandEntryTask: (taskName) => hvigor.isCommandEntryTask(taskName),
      getModuleByScriptPath: (scriptPath) => host._nodeForFile(scriptPath, false)?.coreWrapper,
      getHvigorNodeByScriptPath: (scriptPath) => host._nodeForFile(scriptPath, false)?.wrapper,
    })
    const exports = {
      ...publicUtilities,
      getNode: (scriptPath) => host._nodeForFile(scriptPath || callerFile || host._activeFile, false)?.wrapper,
      getHvigorNode: (scriptPath) =>
        host._nodeForFile(scriptPath || callerFile || host._activeFile, false)?.coreWrapper,
      hvigor,
      hvigorConfig: host._hvigorConfig,
      submitWorker: (node, taskName, workPath, options) => host._submitWorker(node, taskName, workPath, options),
      defaultModelRegistry: host._toolingModelRegistry,
      hvigorCore,
    }
    const guardedExports = new Proxy(exports, {
      get(target, property, receiver) {
        if (typeof property === 'string' && UNSUPPORTED_HVIGOR_INTERNAL_EXPORTS.has(property)) {
          throw host._error(
            'ARKDOWN_UNSUPPORTED_HVIGOR_INTERNAL_EXPORT',
            `@ohos/hvigor internal export ${property} is not part of the declared 6.24.3 API`,
            { exportName: property },
          )
        }
        return Reflect.get(target, property, receiver)
      },
    })
    host._hvigorExports.set(cacheKey, guardedExports)
    return guardedExports
  }

  loadWithModuleMocks(file, callback) {
    const originalLoad = Module._load
    const host = this
    Module._load = function arkdownHvigorModuleLoad(request, parent, isMain) {
      if (request === '@ohos/hvigor') {
        return host._makeHvigorMock(parent?.filename || file)
      }
      if (request === '@ohos/hvigor-ohos-plugin') {
        const owner = host._nodeForFile(parent?.filename, false) || host._nodeForFile(host._activeFile, false)
        return host._makeOhosPluginMock(owner?.file || parent?.filename || file)
      }
      const node =
        typeof request === 'string' && path.isAbsolute(request) ? host._nodeForFile(request, false) : undefined
      if (!node) return originalLoad.call(this, request, parent, isMain)
      host._activeFile = node.file
      host._effects = node.effects
      host._aliasBuildScriptFile(Module._resolveFilename(request, parent, isMain), node.file)
      const loaded = originalLoad.call(this, request, parent, isMain)
      const value = unwrapDefault(loaded)
      node.exportValue = value
      if (
        value &&
        typeof value === 'object' &&
        ['plugins', 'system', 'config'].some((key) => Object.hasOwn(value, key)) &&
        (!value.system || (typeof value.system === 'object' && Object.keys(value.system).length === 0))
      ) {
        const system = () => ({
          __arkdownConfigOnly: true,
          getPluginId: () => '__arkdown_config__',
          getContext: () => undefined,
        })
        if (Object.hasOwn(loaded, 'default')) return { ...loaded, default: { ...value, system } }
        return { default: { ...value, system } }
      }
      if (
        loaded &&
        typeof loaded === 'object' &&
        !Object.hasOwn(loaded, 'default') &&
        ['plugins', 'system', 'config', 'compilePluginPath', 'hvigorPluginFile', 'buildOption'].some((key) =>
          Object.hasOwn(loaded, key),
        )
      ) {
        return { default: loaded }
      }
      return loaded
    }
    let pending = false
    try {
      const result = callback()
      if (result && typeof result.then === 'function') {
        pending = true
        return result.finally(() => {
          Module._load = originalLoad
        })
      }
      return result
    } finally {
      if (!pending) Module._load = originalLoad
    }
  }
}

// Plugin discovery and hook wrapping remain JavaScript glue. Function-valued
// ordering is resolved by the Rust Rolldown planner before TSFNs are created.
function normalizePluginList(value, baseDir) {
  const plugins = []
  appendNormalizedPlugins(plugins, unwrapDefault(value), baseDir)
  return applyFunctionOrder(plugins)
}

// normalizeSortedPluginList is the strict hvigor compile-plugin loader used by
// compilePluginPath / hvigorPluginFile. Unlike Arkdown's explicit plugin API,
// hvigor accepts only an object or a flat array of objects and requires a
// non-empty string name even when the plugin declares no hooks.
function normalizeSortedPluginList(value, _baseDir, pluginFile = '<compile-plugin>', moduleRoot, targetName) {
  const exported = unwrapDefault(value)
  const items = Array.isArray(exported) ? exported : [exported]
  const validation = nativeRuntimeBinding.validateHvigorCompilePlugins(items, pluginFile)
  emitPluginPlanDiagnostics(validation)
  return items.map((plugin) => normalizeHvigorCompilePlugin(plugin, pluginFile, moduleRoot, targetName))
}

/**
 * Thin object-identity glue around the Rust hvigor/Rolldown plugin planner.
 * Rust owns scope grouping, the latest IDE built-in inventory, the sequential
 * order state machine, duplicate validation, and native-slot reduction. The
 * N-API planner invokes function-valued `order` callbacks on this V8 thread.
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
    const response = nativeRuntimeBinding.planHvigorCompilePlugins(
      {
        compileMode: this.config.compileMode,
        preview: Boolean(this.config.preview),
        target: this.config.target,
        mode: this.config.mode || this.config.buildMode,
      },
      compilePlugins,
    )
    if (response.error) {
      emitDiagnostic('error', response.error)
    }
    const retained = new Set(response.placements.map(({ pluginIndex }) => compilePlugins[pluginIndex]))
    return plugins.filter((plugin) => !plugin.__arkdownHvigorCompilePlugin || retained.has(plugin))
  }
}

// Object identity/shape glue for the Rust explicit-package-plugin planner.
// String orders remain on their plugin for PluginFactory; a callback order is
// removed after Rust resolves it into deterministic list order.
function applyFunctionOrder(plugins) {
  const response = nativeRuntimeBinding.planRolldownPackagePlugins(plugins)
  emitPluginPlanDiagnostics(response)
  return response.placements.map((placement) => plugins[placement.pluginIndex])
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
    if (value.__arkdownHvigorSystemKind) return
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
    plugins.push(normalizePluginHooks(value))
    return
  }
  if (typeof value === 'object' && value.path) {
    return
  }
}

function normalizeHvigorCompilePlugin(plugin, pluginFile, moduleRoot, targetName) {
  const normalized = normalizePluginHooks(plugin, { compilePlugin: true, pluginFile })
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

function normalizePluginHooks(plugin, options = {}) {
  const compilePlugin = options.compilePlugin === true
  const pluginFile = options.pluginFile
  const normalized = compilePlugin ? { name: plugin.name } : { ...plugin }
  let pluginShareContext = {}
  const directHooks = compilePlugin
    ? PLUGIN_DISPATCH_HOOKS.filter((hookName) => hookName !== 'resolveDynamicImport')
    : PLUGIN_DISPATCH_HOOKS
  const hookNames = directHooks.filter((hookName) => typeof plugin[hookName] === 'function')
  for (const hookName of ALL_PLUGIN_HOOKS) delete normalized[hookName]
  const wrapResult = (hookName, result) => {
    if (compilePlugin && HVIGOR_DISCARDED_PLUGIN_RESULTS.has(hookName)) return {}
    const wrap = (value) =>
      value == null || !PLUGIN_RESULT_FIELDS[hookName] ? {} : { [PLUGIN_RESULT_FIELDS[hookName]]: value }
    return result && typeof result.then === 'function' ? Promise.resolve(result).then(wrap) : wrap(result)
  }
  const dispatchPluginHook = (request = {}) => {
    if (request.hookName === 'seedContext') {
      pluginShareContext = request.share && typeof request.share === 'object' ? request.share : {}
      return {}
    }
    const hookName = request.hookName
    const moduleInfo = request.moduleInfo
    const hookCtx = moduleInfo?.id ? { moduleInfoMap: { [moduleInfo.id]: moduleInfo } } : {}
    let args = hookName === 'buildStart' ? (compilePlugin ? [] : [request.buildStartOptions]) : undefined
    args ||= PLUGIN_HOOK_ARGUMENTS[hookName]?.(request)
    if (hookName === 'buildEnd') {
      const errors = Array.isArray(request.buildEndContext?.errors) ? request.buildEndContext.errors : []
      const error = errors.length
        ? Object.assign(
            new Error(
              compilePlugin
                ? `Rolldown build ended with ${errors.length} error(s)`
                : errors.map((item) => item.message || String(item)).join('\n'),
            ),
            { name: 'RolldownError', errors },
          )
        : undefined
      args = [error]
    }
    if (!args) throw new Error(`unknown Rust plugin hook ${String(hookName)}`)
    try {
      const result = plugin[hookName].apply(buildPluginContext(hookCtx, pluginShareContext, request.context), args)
      return wrapResult(hookName, result)
    } catch (cause) {
      if (compilePlugin && pluginFile && cause && typeof cause === 'object') {
        cause.message = `${cause.message} ${pluginFile}`
      }
      throw cause
    }
  }

  Object.defineProperties(normalized, {
    __arkdownPluginHooks: { value: hookNames },
    __arkdownDispatchPluginHook: { value: dispatchPluginHook },
  })
  return normalized
}

function emitPluginPlanDiagnostics(response) {
  for (const warning of response.warnings || []) emitDiagnostic('warn', warning)
  if (response.error) emitDiagnostic('error', response.error)
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
  process.emitWarning(message, { code: diagnostic.code, type: 'ArkdownWarning' })
}

// ---------------------------------------------------------------------------
// Plugin context binding — hvigor's doWrapper / getCompilePluginContext
// ---------------------------------------------------------------------------

function reviveOxcPluginAst(envelope) {
  if (!envelope || typeof envelope !== 'object' || !envelope.node) return undefined
  const program = envelope.node
  for (const fixPath of envelope.fixes || []) {
    let node = program
    for (const key of fixPath) node = node[key]
    if (node.bigint !== undefined) {
      node.value = BigInt(node.bigint)
    } else if (node.regex) {
      try {
        node.value = new RegExp(node.regex.pattern, node.regex.flags)
      } catch {
        // Match OXC/Rollup: retain null when the current Node runtime cannot
        // construct a regular expression accepted by the parser.
      }
    }
  }
  return program
}

function normalizePluginParseError(cause) {
  const message = cause instanceof Error ? cause.message : String(cause)
  const parsed = /PLUGIN_PARSE_ERROR: (.*) \((\d+):(\d+)\) \[pos=(\d+)\]/.exec(message)
  if (!parsed) return cause
  const error = new SyntaxError(`${parsed[1]} (${parsed[2]}:${parsed[3]})`)
  error.code = 'PLUGIN_PARSE_ERROR'
  error.pos = Number(parsed[4])
  error.loc = { line: Number(parsed[2]), column: Number(parsed[3]) }
  return error
}

function parsePluginAst(source, options = {}, rolldownContext) {
  const nativeOptions = {
    sourceType: options.sourceType || 'module',
    ranges: options.ranges === true,
    locations: options.locations === true,
    sourceFile: options.sourceFile,
    directSourceFile: options.directSourceFile,
    allowReturnOutsideFunction: options.allowReturnOutsideFunction === true,
    preserveParens: options.preserveParens === true,
    captureComments: options.onComment !== undefined && options.onComment !== null,
    ecmaVersion: options.ecmaVersion ?? 'latest',
    allowAwaitOutsideFunction: options.allowAwaitOutsideFunction === true,
    allowSuperOutsideMethod: options.allowSuperOutsideMethod === true,
    allowHashBang: options.allowHashBang,
    hasOnToken: options.onToken !== undefined && options.onToken !== null,
    hasProgram: options.program !== undefined && options.program !== null,
  }
  let result
  try {
    result =
      rolldownContext && typeof rolldownContext.parse === 'function'
        ? rolldownContext.parse(source, nativeOptions)
        : nativeRuntimeBinding.parsePluginAst(source, nativeOptions)
  } catch (cause) {
    throw normalizePluginParseError(cause)
  }
  const comments = result.comments || []
  if (Array.isArray(options.onComment)) {
    options.onComment.push(...comments)
  } else if (typeof options.onComment === 'function') {
    for (const comment of comments) {
      options.onComment.call(
        options,
        comment.type === 'Block',
        comment.value,
        comment.start,
        comment.end,
        comment.loc?.start,
        comment.loc?.end,
      )
    }
  } else if (options.onComment !== undefined && options.onComment !== null) {
    throw new TypeError('onComment must be a function or an array')
  }
  return reviveOxcPluginAst(result.ast)
}

/**
 * Build a full hvigor-compatible compile plugin context object.
 *
 * Merges the per-plugin share data seeded before Rolldown starts with the
 * current typed native context.
 *
 * Shape: `{ share: { bundleName, buildMode, … }, getModuleIds(), getModuleInfo(), … }`
 *
 * # Upstream correspondence
 *
 * Corresponds to hvigor's `getCompilePluginContext(this)` result from
 * `arkts-pack.ts`.  User plugins access `this.share.projectConfig.bundleName`,
 * `this.getModuleIds()`, etc., through this object.
 */
function buildPluginContext(hookCtx, fallbackShare = {}, rolldownContext) {
  const share = hookCtx.share || fallbackShare
  const moduleIds = hookCtx.moduleIds || []
  const moduleInfoMap = hookCtx.moduleInfoMap || {}
  const fileNames = hookCtx.fileNames || {}
  return {
    share,
    getModuleIds() {
      return rolldownContext ? rolldownContext.getModuleIds().values() : moduleIds.values()
    },
    getModuleInfo(id) {
      return rolldownContext ? rolldownContext.getModuleInfo(id) : moduleInfoMap[id] || null
    },
    parse(source, options = {}) {
      return parsePluginAst(source, options, rolldownContext)
    },
    getFileName(referenceId) {
      if (rolldownContext) return rolldownContext.getFileName(referenceId)
      if (Object.hasOwn(fileNames, referenceId)) return fileNames[referenceId]
      const error = new Error(`Unable to get file name for unknown reference ID "${referenceId}"`)
      error.code = 'FILE_NAME_NOT_FOUND'
      throw error
    },
  }
}

function parseShouldInvalidCacheRequest(request) {
  const parsed = request && typeof request === 'object' ? request : { cacheHit: Boolean(request) }
  if (parsed && typeof parsed === 'object' && parsed.__arkdownOxcAst) {
    if (parsed.ast === undefined) parsed.ast = reviveOxcPluginAst(parsed.__arkdownOxcAst)
    delete parsed.__arkdownOxcAst
  }
  return parsed
}

function hasRollupHook(plugin) {
  return Array.from(ALL_PLUGIN_HOOKS).some((hook) => typeof plugin[hook] === 'function')
}

function configureHvigorNodePath(projectRoot, plannedWorkspace, plannedNodeModules) {
  if (!plannedWorkspace || !plannedNodeModules) {
    const error = new Error(`Rust returned an incomplete Hvigor dependency workspace plan for ${projectRoot}`)
    error.code = 'ARKDOWN_HVIGOR_WORKSPACE_PLAN_INCOMPLETE'
    throw error
  }
  process.env.WORKSPACE_DIR = plannedWorkspace
  const nodePathEntries = (process.env.NODE_PATH || '').split(path.delimiter).filter(Boolean)
  process.env.NODE_PATH = [plannedNodeModules, ...nodePathEntries.filter((entry) => entry !== plannedNodeModules)].join(
    path.delimiter,
  )
  Module._initPaths()
  installHvigorTypeScriptExtension()
  return plannedNodeModules
}

function getHvigorUserHome() {
  if (process.env.HVIGOR_USER_HOME && path.isAbsolute(process.env.HVIGOR_USER_HOME)) {
    return process.env.HVIGOR_USER_HOME
  }
  return path.resolve(os.homedir(), '.hvigor')
}

function hvigorProjectHash(projectRoot) {
  return nativeRuntimeBinding.hvigorProjectCacheHash(path.resolve(projectRoot))
}

function installHvigorTypeScriptExtension() {
  if (hvigorTypeScriptExtensionInstalled) {
    return
  }
  const extension = function hvigorTypeScriptExtension(mod, filename) {
    mod._compile(compileHvigorScriptSync(filename), filename)
  }
  // This is the exact extension pair installed by latest hvigor's
  // base/boot/hooks/require-hook.js.
  Module._extensions['.ts'] = extension
  Module._extensions['.mjs'] = extension
  hvigorTypeScriptExtensionInstalled = true
}

function markHvigorScriptCompileError(error) {
  if (error && typeof error === 'object') {
    error.code = 'HVIGOR_TYPESCRIPT_DIAGNOSTIC'
  }
  return error
}

function compileHvigorScriptSync(filename, sourceMap = false) {
  try {
    return nativeRuntimeBinding.compileHvigorScriptSync(filename, sourceMap)
  } catch (error) {
    throw markHvigorScriptCompileError(error)
  }
}

async function compileHvigorScript(filename, sourceMap = false) {
  try {
    return await nativeRuntimeBinding.compileHvigorScript(filename, sourceMap)
  } catch (error) {
    throw markHvigorScriptCompileError(error)
  }
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
  if (host) {
    host._aliasBuildScriptFile(resolved, file)
  }
  delete require.cache[resolved]
  return host ? host.loadWithModuleMocks(file, () => require(resolved)) : require(resolved)
}

// host: optional HvigorRuntimeHost for capturing task-factory config effects.
async function loadHvigorScriptModule(file, host) {
  installHvigorTypeScriptExtension()
  const output = await compileHvigorScript(file)

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

function toArray(value) {
  if (value == null) {
    return []
  }
  return Array.isArray(value) ? value : [value]
}

function mergeHvigorConfig(base, overlay) {
  return nativeRuntimeBinding.mergeHvigorConfig(base, overlay)
}

module.exports = {
  HVIGOR_PARITY_VERSION,
  HvigorRuntimeHost,
  HvigorCompilePluginPlanner,
  RuntimePluginLoader,
  createBuild,
  createHvigorCommand,
  normalizePluginList,
  normalizeSortedPluginList,
  applyFunctionOrder,
  parseShouldInvalidCacheRequest,
  buildPluginContext,
  requireDefaultExport,
  hvigorProjectHash,
}
