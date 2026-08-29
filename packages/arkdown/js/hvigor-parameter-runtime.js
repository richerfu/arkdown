'use strict'

// V8 identity bridge for Rust-owned Hvigor parameter state.

function createHvigorParameterUtilities(nativeRuntimeBinding, plan) {
  const state = new nativeRuntimeBinding.RuntimeHvigorParameterState()
  const v8Values = new Map()

  const releaseToken = (token) => {
    if (token !== undefined && token !== null) v8Values.delete(token)
  }
  const releaseTokens = (tokens) => {
    for (const token of tokens || []) releaseToken(token)
  }
  const readEntry = (scope, key) => {
    const entry = state.entry(scope, key)
    if (!entry.exists || entry.kind === 'undefined') return undefined
    if (entry.kind === 'v8') return v8Values.get(entry.token)
    return entry.value
  }
  const normalizeEntry = (scope, key, value) => {
    if (scope === 'startParams' && key === 'logLevel') {
      return String(value?.levelStr || value || 'info').toLowerCase()
    }
    return value
  }
  const writeEntry = (scope, key, rawValue) => {
    const value = normalizeEntry(scope, key, rawValue)
    if (value === undefined) {
      releaseToken(state.setUndefinedEntry(scope, key))
      return true
    }
    if ((typeof value === 'object' && value !== null) || typeof value === 'function') {
      const mutation = state.setV8Entry(scope, key)
      releaseToken(mutation.replacedToken)
      v8Values.set(mutation.token, value)
      return true
    }
    releaseToken(state.setJsonEntry(scope, key, value))
    return true
  }
  const clearScope = (scope) => releaseTokens(state.clearScope(scope))
  const replaceScope = (scope, value, undefinedKeys = []) => {
    clearScope(scope)
    for (const [key, entry] of Object.entries(value || {})) writeEntry(scope, key, entry)
    for (const key of undefinedKeys) writeEntry(scope, key, undefined)
  }
  const snapshotScope = (scope) => {
    const snapshot = {}
    for (const key of state.keys(scope)) snapshot[key] = readEntry(scope, key)
    return snapshot
  }
  const createScopeProxy = (scope) => {
    const target = {}
    const localDescriptors = new Map()
    return new Proxy(target, {
      get(_target, key, receiver) {
        if (typeof key !== 'string' || localDescriptors.has(key)) return Reflect.get(target, key, receiver)
        return readEntry(scope, key)
      },
      set(_target, key, value, receiver) {
        if (typeof key !== 'string' || localDescriptors.has(key)) return Reflect.set(target, key, value, receiver)
        return writeEntry(scope, key, value)
      },
      has(_target, key) {
        if (typeof key !== 'string' || localDescriptors.has(key)) return Reflect.has(target, key)
        return state.entry(scope, key).exists
      },
      ownKeys() {
        const nativeKeys = state.keys(scope)
        return [...nativeKeys, ...Reflect.ownKeys(target).filter((key) => !nativeKeys.includes(key))]
      },
      getOwnPropertyDescriptor(_target, key) {
        const local = Reflect.getOwnPropertyDescriptor(target, key)
        if (local || typeof key !== 'string' || !state.entry(scope, key).exists) return local
        return { configurable: true, enumerable: true, writable: true, value: readEntry(scope, key) }
      },
      defineProperty(_target, key, descriptor) {
        if (typeof key === 'string' && 'value' in descriptor && descriptor.configurable !== false) {
          localDescriptors.delete(key)
          return writeEntry(scope, key, descriptor.value)
        }
        if (typeof key === 'string') {
          releaseToken(state.deleteEntry(scope, key))
          localDescriptors.set(key, true)
        }
        return Reflect.defineProperty(target, key, descriptor)
      },
      deleteProperty(_target, key) {
        if (typeof key !== 'string' || localDescriptors.has(key)) {
          localDescriptors.delete(key)
          return Reflect.deleteProperty(target, key)
        }
        releaseToken(state.deleteEntry(scope, key))
        return true
      },
    })
  }

  const properties = createScopeProxy('properties')
  const extParams = createScopeProxy('extParams')
  const startParams = createScopeProxy('startParams')

  class CoreParameter {
    get properties() {
      return properties
    }

    set properties(value) {
      replaceScope('properties', value)
    }

    get extParams() {
      return extParams
    }

    set extParams(value) {
      replaceScope('extParams', value)
    }

    get startParams() {
      return startParams
    }

    get workspaceDir() {
      return state.workspaceDir
    }

    set workspaceDir(value) {
      state.workspaceDir = String(value || '')
    }

    clean() {
      releaseTokens(state.clean())
    }
  }

  const cloneDeep = (value) => structuredClone(value)
  const freezeDeep = (value) => {
    if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value
    for (const child of Object.values(value)) freezeDeep(child)
    return Object.freeze(value)
  }
  const immutableSnapshot = (scope) => freezeDeep(cloneDeep(snapshotScope(scope)))

  class ExternalParameter {
    getExtParam(key) {
      return this.getExtParamInternal(key)
    }

    getExtParamInternal(key) {
      return immutableSnapshot('extParams')[key]
    }

    getExtParams() {
      return immutableSnapshot('extParams')
    }

    getExtParamsInternal() {
      return immutableSnapshot('extParams')
    }

    _getExtParams() {
      return immutableSnapshot('extParams')
    }

    getProperties() {
      return immutableSnapshot('properties')
    }

    getPropertiesInternal() {
      return immutableSnapshot('properties')
    }

    _getProperties() {
      return immutableSnapshot('properties')
    }

    getProperty(key) {
      return this.getPropertyInternal(key)
    }

    getPropertyInternal(key) {
      return immutableSnapshot('properties')[key]
    }

    _getProperty(key) {
      return this.getPropertyInternal(key)
    }

    setProperty(key, value) {
      return this.setPropertyInternal(key, value)
    }

    setPropertyInternal(key, value) {
      try {
        const nativeValue = ['undefined', 'function', 'symbol', 'bigint'].includes(typeof value) ? null : value
        state.validateProperty(String(key), nativeValue, typeof value)
      } catch (cause) {
        cause.code = 'ARKDOWN_HVIGOR_PARAMETER_VALIDATION_FAILED'
        throw cause
      }
      writeEntry('properties', String(key), value)
    }

    _setProperty(key, value) {
      return this.setPropertyInternal(key, value)
    }

    getCallerPath() {
      const line = new Error().stack?.split('\n').find((entry) => entry.trim().startsWith('at plugin'))
      const match = line?.match(/\((.*?):\d+:\d+\)$/)
      return match?.[1] ? `At file: ${match[1]}` : ''
    }

    getStartParams() {
      return this._getStartParams()
    }

    getStartParamsInternal() {
      return this._getStartParams()
    }

    _getStartParams() {
      const snapshot = state.publicStartSnapshot()
      if (snapshot.analyze === null) snapshot.analyze = undefined
      return freezeDeep(snapshot)
    }

    getWorkspaceDir() {
      return state.workspaceDir
    }

    getWorkspaceDirInternal() {
      return this._getWorkspaceDir()
    }

    _getWorkspaceDir() {
      return state.workspaceDir
    }
  }

  const coreParameter = new CoreParameter()
  Object.defineProperties(coreParameter, {
    _properties: {
      configurable: true,
      enumerable: true,
      get: () => properties,
      set: (value) => replaceScope('properties', value),
    },
    _extParams: {
      configurable: true,
      enumerable: true,
      get: () => extParams,
      set: (value) => replaceScope('extParams', value),
    },
    _startParams: {
      configurable: true,
      enumerable: true,
      get: () => startParams,
      set: (value) => replaceScope('startParams', value),
    },
    _workspaceDir: {
      configurable: true,
      enumerable: true,
      get: () => state.workspaceDir,
      set: (value) => {
        state.workspaceDir = String(value || '')
      },
    },
  })
  replaceScope('properties', plan.properties, plan.undefinedPropertyKeys)
  replaceScope('extParams', plan.extParams, plan.undefinedExtParamKeys)
  for (const [key, value] of Object.entries(plan.coreStartParams || {})) writeEntry('startParams', key, value)
  coreParameter.workspaceDir = plan.workspaceDir
  const parameter = new ExternalParameter()

  const globalDataState = new nativeRuntimeBinding.RuntimeHvigorGlobalData()
  class GlobalData {
    init(cliEnv, cliOpts) {
      this.cliEnv = cliEnv
      this.cliOpts = cliOpts
      this.buildId = globalDataState.init()
    }

    clean() {
      globalDataState.clean()
      this.buildId = undefined
    }
  }
  const globalData = new GlobalData()

  return {
    coreParameter,
    globalData,
    parameter,
    state,
  }
}

module.exports = { createHvigorParameterUtilities }
