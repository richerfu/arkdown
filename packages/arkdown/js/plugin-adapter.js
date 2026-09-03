'use strict'

const Module = require('node:module')
const path = require('node:path')
const { pathToFileURL } = require('node:url')

const nativeBinding = require('../index.js')

// This is the complete user-authored compile-plugin surface consumed by the
// ArkTS 1.1 module pipeline. The standard hooks run inside Rolldown; the two
// custom lifecycle hooks are scheduled by Rust at their source-defined stage.
const PLUGIN_HOOKS = Object.freeze([
  'buildStart',
  'resolveId',
  'load',
  'transform',
  'moduleParsed',
  'resolveDynamicImport',
  'beforeBuildEnd',
  'buildEnd',
  'shouldInvalidCache',
])

const COMPILE_PLUGIN_HOOKS = Object.freeze(PLUGIN_HOOKS.filter((name) => name !== 'resolveDynamicImport'))
const RESULT_FIELDS = Object.freeze({
  resolveId: 'resolveId',
  resolveDynamicImport: 'resolveId',
  load: 'load',
  transform: 'transform',
  shouldInvalidCache: 'shouldInvalidCache',
})
const TYPESCRIPT_EXTENSIONS = new Set(['.ts', '.mts', '.cts'])
const JAVASCRIPT_EXTENSIONS = new Set(['.js', '.mjs', '.cjs'])
const UNSUPPORTED_ROLLUP_HOOKS = new Set([
  'options',
  'outputOptions',
  'watchChange',
  'closeWatcher',
  'shouldTransformCachedModule',
  'renderStart',
  'banner',
  'footer',
  'intro',
  'outro',
  'renderDynamicImport',
  'resolveFileUrl',
  'resolveImportMeta',
  'renderChunk',
  'augmentChunkHash',
  'generateBundle',
  'writeBundle',
  'closeBundle',
])
const REMOVED_HVIGOR_CONFIG_KEYS = new Set([
  'tasks',
  'task',
  'executionMode',
  'prop',
  'configOverrides',
  'hvigorProperties',
  'extParams',
  'sync',
  'typeCheck',
  'enableBuildScriptTypeCheck',
  'parallel',
  'daemon',
])

let esmLoadSequence = 0

function arkdownError(code, message, details = {}) {
  const error = new Error(message)
  error.code = code
  Object.assign(error, details)
  return error
}

function emitDiagnostic(level, diagnostic) {
  if (level === 'error') {
    throw arkdownError(diagnostic.code, diagnostic.message, {
      ...(diagnostic.pluginName ? { pluginName: diagnostic.pluginName } : {}),
    })
  }
  process.emitWarning(`[arkdown] ${diagnostic.code}: ${diagnostic.message}`, {
    code: diagnostic.code,
    type: 'ArkdownWarning',
  })
}

function emitPlanDiagnostics(response) {
  for (const warning of response.warnings || []) emitDiagnostic('warning', warning)
  if (response.error) emitDiagnostic('error', response.error)
}

function unwrapDefault(value) {
  return value && typeof value === 'object' && 'default' in value ? value.default : value
}

function requireDefaultExport(value, file) {
  if (value && typeof value === 'object' && 'default' in value && value.default) return value.default
  throw arkdownError('PLUGIN_FILE_NO_DEFAULT_EXPORT', `Plugin module must have a default export: ${file}`, {
    pluginFile: file,
  })
}

function asArray(value) {
  if (value === undefined || value === null) return []
  return Array.isArray(value) ? value : [value]
}

function validateHookSurface(plugin, hookNames, pluginFile) {
  for (const hookName of UNSUPPORTED_ROLLUP_HOOKS) {
    if (plugin[hookName] === undefined || plugin[hookName] === null) continue
    throw arkdownError(
      'ARKDOWN_PLUGIN_HOOK_UNSUPPORTED',
      `Plugin "${String(plugin.name || '<anonymous>')}" uses unsupported hook "${hookName}"`,
      { hookName, pluginFile },
    )
  }
  for (const hookName of hookNames) {
    const hook = plugin[hookName]
    if (hook === undefined || hook === null) continue
    if (typeof hook !== 'function') {
      throw arkdownError(
        'ARKDOWN_PLUGIN_HOOK_INVALID',
        `Plugin "${String(plugin.name || '<anonymous>')}" hook "${hookName}" must be a function`,
        { hookName, pluginFile },
      )
    }
  }
}

function reviveOxcAst(envelope) {
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
        // Keep OXC's null value when the current Node runtime rejects the
        // parser-accepted regular expression.
      }
    }
  }
  return program
}

function normalizeParseError(cause) {
  const message = cause instanceof Error ? cause.message : String(cause)
  const parsed = /PLUGIN_PARSE_ERROR: (.*) \((\d+):(\d+)\) \[pos=(\d+)\]/.exec(message)
  if (!parsed) return cause
  const error = new SyntaxError(`${parsed[1]} (${parsed[2]}:${parsed[3]})`)
  error.code = 'PLUGIN_PARSE_ERROR'
  error.pos = Number(parsed[4])
  error.loc = { line: Number(parsed[2]), column: Number(parsed[3]) }
  return error
}

function parsePluginAst(source, options, context, binding) {
  const parseOptions = options || {}
  const nativeOptions = {
    sourceType: parseOptions.sourceType || 'module',
    ranges: parseOptions.ranges === true,
    locations: parseOptions.locations === true,
    sourceFile: parseOptions.sourceFile,
    directSourceFile: parseOptions.directSourceFile,
    allowReturnOutsideFunction: parseOptions.allowReturnOutsideFunction === true,
    preserveParens: parseOptions.preserveParens === true,
    captureComments: parseOptions.onComment !== undefined && parseOptions.onComment !== null,
    ecmaVersion: parseOptions.ecmaVersion ?? 'latest',
    allowAwaitOutsideFunction: parseOptions.allowAwaitOutsideFunction === true,
    allowSuperOutsideMethod: parseOptions.allowSuperOutsideMethod === true,
    allowHashBang: parseOptions.allowHashBang,
    hasOnToken: parseOptions.onToken !== undefined && parseOptions.onToken !== null,
    hasProgram: parseOptions.program !== undefined && parseOptions.program !== null,
  }
  let result
  try {
    result = context ? context.parse(source, nativeOptions) : binding.parsePluginAst(source, nativeOptions)
  } catch (cause) {
    throw normalizeParseError(cause)
  }
  const comments = result.comments || []
  if (Array.isArray(parseOptions.onComment)) {
    parseOptions.onComment.push(...comments)
  } else if (typeof parseOptions.onComment === 'function') {
    for (const comment of comments) {
      parseOptions.onComment.call(
        parseOptions,
        comment.type === 'Block',
        comment.value,
        comment.start,
        comment.end,
        comment.loc?.start,
        comment.loc?.end,
      )
    }
  } else if (parseOptions.onComment !== undefined && parseOptions.onComment !== null) {
    throw new TypeError('onComment must be a function or an array')
  }
  return reviveOxcAst(result.ast)
}

function pluginContext(request, share, binding) {
  const context = request.context
  const moduleInfo = request.moduleInfo
  return {
    share,
    getModuleIds() {
      if (context) return context.getModuleIds().values()
      return moduleInfo?.id ? [moduleInfo.id].values() : [][Symbol.iterator]()
    },
    getModuleInfo(id) {
      if (context) return context.getModuleInfo(id)
      return moduleInfo?.id === id ? moduleInfo : null
    },
    parse(source, options = {}) {
      return parsePluginAst(source, options, context, binding)
    },
    getFileName(referenceId) {
      if (context) return context.getFileName(referenceId)
      throw arkdownError('FILE_NAME_NOT_FOUND', `Unable to get file name for unknown reference ID "${referenceId}"`)
    },
  }
}

function cacheRequest(request) {
  const value = request && typeof request === 'object' ? request : { cacheHit: Boolean(request) }
  if (value.__arkdownOxcAst) {
    if (value.ast === undefined) value.ast = reviveOxcAst(value.__arkdownOxcAst)
    delete value.__arkdownOxcAst
  }
  return value
}

function hookArguments(hookName, request, compilePlugin) {
  switch (hookName) {
    case 'buildStart':
      return compilePlugin ? [] : [request.buildStartOptions]
    case 'resolveId':
    case 'resolveDynamicImport':
      return [request.id, request.importer, request.resolveOptions]
    case 'load':
      return [request.id]
    case 'transform':
      return [request.code, request.id]
    case 'moduleParsed':
      return [request.moduleInfo]
    case 'beforeBuildEnd':
      return [undefined]
    case 'shouldInvalidCache':
      return [cacheRequest(request.cacheRequest)]
    case 'buildEnd': {
      const errors = request.buildEndContext?.errors || []
      if (errors.length === 0) return [undefined]
      const error = new Error(errors.map((item) => item.message || String(item)).join('\n'))
      error.name = 'RolldownError'
      error.errors = errors
      return [error]
    }
    default:
      throw arkdownError('ARKDOWN_PLUGIN_HOOK_UNKNOWN', `Unknown native plugin hook "${String(hookName)}"`)
  }
}

function normalizePlugin(plugin, binding, options = {}) {
  const compilePlugin = options.compilePlugin === true
  const pluginFile = options.pluginFile
  const allowedHooks = compilePlugin ? COMPILE_PLUGIN_HOOKS : PLUGIN_HOOKS
  validateHookSurface(plugin, allowedHooks, pluginFile)

  const normalized = compilePlugin ? { name: plugin.name } : { ...plugin }
  const hookNames = allowedHooks.filter((name) => typeof plugin[name] === 'function')
  for (const hookName of PLUGIN_HOOKS) delete normalized[hookName]
  let share = {}

  const dispatch = (request = {}) => {
    if (request.hookName === 'seedContext') {
      share = request.share && typeof request.share === 'object' ? request.share : {}
      return {}
    }
    const hookName = request.hookName
    const result = plugin[hookName].apply(pluginContext(request, share, binding), hookArguments(hookName, request, compilePlugin))
    const field = RESULT_FIELDS[hookName]
    if (!field) return result && typeof result.then === 'function' ? Promise.resolve(result).then(() => ({})) : {}
    const wrap = (value) => (value === undefined || value === null ? {} : { [field]: value })
    return result && typeof result.then === 'function' ? Promise.resolve(result).then(wrap) : wrap(result)
  }

  Object.defineProperties(normalized, {
    __arkdownPluginHooks: { value: hookNames },
    __arkdownDispatchPluginHook: { value: dispatch },
  })
  if (compilePlugin) {
    Object.defineProperties(normalized, {
      __arkdownHvigorCompilePlugin: { value: true },
      __arkdownHvigorOrder: { value: plugin.order },
      ...(options.moduleRoot ? { __arkdownModuleRoot: { value: path.resolve(options.moduleRoot) } } : {}),
      ...(options.targetName ? { __arkdownTargetName: { value: options.targetName } } : {}),
    })
  }
  return normalized
}

function normalizePluginList(value, binding = nativeBinding) {
  const plugins = []
  const append = (candidate) => {
    const plugin = unwrapDefault(candidate)
    if (plugin === undefined || plugin === null || plugin === false) return
    if (Array.isArray(plugin)) {
      for (const item of plugin) append(item)
      return
    }
    if (typeof plugin === 'function') {
      append(plugin())
      return
    }
    if (typeof plugin !== 'object') {
      throw arkdownError('ARKDOWN_PLUGIN_EXPORT_INVALID', `Expected a plugin object, got ${typeof plugin}`)
    }
    validateHookSurface(plugin, PLUGIN_HOOKS)
    if (!PLUGIN_HOOKS.some((name) => typeof plugin[name] === 'function')) {
      throw arkdownError(
        'ARKDOWN_PLUGIN_EXPORT_INVALID',
        `Plugin "${String(plugin.name || '<anonymous>')}" does not declare a supported hook`,
      )
    }
    plugins.push(normalizePlugin(plugin, binding))
  }
  append(value)
  if (plugins.length === 0) return plugins
  const response = binding.planRolldownPackagePlugins(plugins)
  emitPlanDiagnostics(response)
  return response.placements.map(({ pluginIndex }) => plugins[pluginIndex])
}

class PluginModuleLoader {
  constructor(binding = nativeBinding) {
    this.binding = binding
    this.loaded = new Set()
    this.installTypeScriptExtensions()
  }

  installTypeScriptExtensions() {
    for (const extension of TYPESCRIPT_EXTENSIONS) {
      Module._extensions[extension] = (module, filename) => {
        let output
        try {
          output = this.binding.compilePluginModuleSync(filename, true)
        } catch (cause) {
          if (cause && typeof cause === 'object') cause.code ||= 'ARKDOWN_PLUGIN_MODULE_COMPILE_ERROR'
          throw cause
        }
        module._compile(output, filename)
      }
    }
  }

  resolve(candidate, baseDir) {
    if (typeof candidate !== 'string' || candidate.length === 0) {
      throw arkdownError('ARKDOWN_PLUGIN_PATH_INVALID', 'Plugin path must be a non-empty string')
    }
    try {
      const pluginPath = path.isAbsolute(candidate) ? candidate : path.resolve(baseDir, candidate)
      return require.resolve(pluginPath)
    } catch (cause) {
      throw arkdownError('ARKDOWN_PLUGIN_MODULE_NOT_FOUND', `Cannot resolve plugin module "${candidate}" from ${baseDir}`, {
        cause,
        pluginPath: candidate,
      })
    }
  }

  async load(candidate, baseDir) {
    const file = this.resolve(candidate, baseDir)
    if (this.loaded.has(file)) return null
    this.loaded.add(file)
    const extension = path.extname(file).toLowerCase()
    if (TYPESCRIPT_EXTENSIONS.has(extension)) {
      let output
      try {
        output = await this.binding.compilePluginModule(file, true)
      } catch (cause) {
        if (cause && typeof cause === 'object') cause.code ||= 'ARKDOWN_PLUGIN_MODULE_COMPILE_ERROR'
        throw cause
      }
      const loadedModule = new Module(file, module.parent || module)
      loadedModule.filename = file
      loadedModule.paths = Module._nodeModulePaths(path.dirname(file)).concat(Module.globalPaths)
      loadedModule._compile(output, file)
      return { file, exports: loadedModule.exports }
    }
    if (!JAVASCRIPT_EXTENSIONS.has(extension)) {
      throw arkdownError(
        'ARKDOWN_PLUGIN_MODULE_EXTENSION_UNSUPPORTED',
        `Unsupported plugin module extension "${extension || '<none>'}": ${file}`,
        { pluginFile: file },
      )
    }
    if (extension === '.mjs') return { file, exports: await this.importFresh(file) }
    try {
      delete require.cache[file]
      return { file, exports: require(file) }
    } catch (cause) {
      if (cause?.code !== 'ERR_REQUIRE_ESM') throw cause
      return { file, exports: await this.importFresh(file) }
    }
  }

  importFresh(file) {
    const url = pathToFileURL(file)
    url.searchParams.set('arkdown-plugin-load', String(++esmLoadSequence))
    return import(url.href)
  }
}

class PluginAdapter {
  constructor(binding = nativeBinding) {
    this.binding = binding
    this.loader = new PluginModuleLoader(binding)
  }

  async prepare(config = {}, explicitPlugins) {
    if (!config || typeof config !== 'object' || Array.isArray(config)) {
      throw arkdownError('ARKDOWN_BUILD_CONFIG_INVALID', 'Build config must be an object')
    }
    for (const key of REMOVED_HVIGOR_CONFIG_KEYS) {
      if (Object.hasOwn(config, key)) {
        throw arkdownError(
          'ARKDOWN_HVIGOR_RUNTIME_UNSUPPORTED',
          `Build option "${key}" belonged to the removed Hvigor task runtime; use the native build config instead`,
          { option: key },
        )
      }
    }

    const projectRoot = path.resolve(config.project || process.cwd())
    const compilePlugins = []
    for (const key of ['hvigorPluginFile', 'compilePluginPath']) {
      for (const candidate of asArray(config[key])) {
        const loaded = await this.loader.load(candidate, projectRoot)
        if (!loaded) continue
        const value = requireDefaultExport(loaded.exports, loaded.file)
        const items = Array.isArray(value) ? value : [value]
        const validation = this.binding.validateHvigorCompilePlugins(items, loaded.file)
        emitPlanDiagnostics(validation)
        for (const plugin of items) {
          compilePlugins.push(
            normalizePlugin(plugin, this.binding, {
              compilePlugin: true,
              pluginFile: loaded.file,
            }),
          )
        }
      }
    }
    if (compilePlugins.length > 0) {
      const response = this.binding.planHvigorCompilePlugins(
        {
          compileMode: 'esmodule',
          preview: Boolean(config.preview),
          target: config.target,
          mode: config.mode,
        },
        compilePlugins,
      )
      emitPlanDiagnostics(response)
      const retained = new Set(response.placements.map(({ pluginIndex }) => pluginIndex))
      for (let index = compilePlugins.length - 1; index >= 0; index -= 1) {
        if (!retained.has(index)) compilePlugins.splice(index, 1)
      }
    }

    const nativeConfig = { ...config }
    delete nativeConfig.hvigorPluginFile
    delete nativeConfig.compilePluginPath
    return {
      config: nativeConfig,
      plugins: [...compilePlugins, ...normalizePluginList(explicitPlugins, this.binding)],
    }
  }
}

function createBuild(nativeBuild = nativeBinding.build, binding = nativeBinding) {
  return async (config = {}, plugins) => {
    const prepared = await new PluginAdapter(binding).prepare(config, plugins)
    return nativeBuild(prepared.config, prepared.plugins)
  }
}

module.exports = {
  PLUGIN_HOOKS,
  PluginAdapter,
  PluginModuleLoader,
  createBuild,
  normalizePlugin,
  normalizePluginList,
  parsePluginAst,
  requireDefaultExport,
}
