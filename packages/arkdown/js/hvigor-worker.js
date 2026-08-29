'use strict'

const path = require('node:path')
const { parentPort, workerData } = require('node:worker_threads')
const nativeRuntimeBinding = require('../index.js')

const CUSTOM_WORKER_REQUEST = 'CUSTOM_WORKER_REQUEST'

class WorkerCacheStore {
  constructor(manager, name, pinned) {
    this.manager = manager
    this.name = name
    this.pinned = pinned
    this.values = new Map()
  }

  getCache(key) {
    this.manager.nativeManager.access(this.name, String(key))
    return this.values.get(key)
  }

  get(key) {
    return this.getCache(key)
  }

  setCache(key, value, pinned = false) {
    this.values.set(key, value)
    const eviction = this.manager.nativeManager.set(this.name, String(key), pinned)
    if (eviction) this.manager.evict(eviction)
    return this
  }

  set(key, value) {
    return this.setCache(key, value)
  }

  hasCache(key) {
    return this.values.has(key)
  }

  clear() {
    for (const key of this.values.keys()) this.manager.nativeManager.delete(this.name, String(key))
    this.values.clear()
  }

  delete(key) {
    this.manager.nativeManager.delete(this.name, String(key))
    return this.values.delete(key)
  }

  size() {
    return this.values.size
  }

  keys() {
    return this.values.keys()
  }
}

class WorkerCacheManager {
  constructor(capacity = 4) {
    this.nativeManager = new nativeRuntimeBinding.RuntimeHvigorCacheStoreManager({ capacity }, true)
    this.stores = new Map()
  }

  mount(key, pinned = false) {
    if (!this.stores.has(key)) {
      this.nativeManager.mount(key, pinned)
      this.stores.set(key, new WorkerCacheStore(this, key, pinned))
    }
    return this.stores.get(key)
  }

  unmount(key) {
    const store = this.stores.get(key)
    store?.clear()
    this.nativeManager.unmount(key)
    return this.stores.delete(key)
  }

  clear() {
    for (const store of this.stores.values()) store.values.clear()
    this.nativeManager.clear()
    this.stores.clear()
  }

  size() {
    return this.stores.size
  }

  keys() {
    return this.stores.keys()
  }

  cacheItemSize() {
    let size = 0
    for (const store of this.stores.values()) size += store.size()
    return size
  }

  evict({ storeName, key }) {
    const store = this.stores.get(storeName)
    if (!store) return
    for (const candidate of store.values.keys()) {
      if (String(candidate) === key) {
        store.values.delete(candidate)
        return
      }
    }
  }
}

class HvigorWorkerRuntime {
  constructor(port) {
    this.port = port
    this.cacheManager = new WorkerCacheManager()
  }

  start() {
    this.port.on(CUSTOM_WORKER_REQUEST, (request) => {
      this.port.postMessage({ ...request, event: CUSTOM_WORKER_REQUEST })
    })
    this.port.on('message', (message) => this.handle(message))
  }

  workFunction(workPath) {
    const suffix = workPath.indexOf('.js')
    if (suffix <= 0) return undefined
    const moduleEnd = suffix + '.js'.length
    const modulePath = workPath.slice(0, moduleEnd)
    const selectors = workPath.slice(moduleEnd + 1).split(path.sep)
    const loaded = require(modulePath)
    const namespace = loaded?.__esModule ? loaded : Object.assign({}, loaded, { default: loaded })
    if (selectors.length === 0 || selectors[0] === '') return namespace.default
    let selected = namespace
    for (let index = 0; index < selectors.length; index++) {
      const selector = selectors[index]
      if (selector === '') {
        if (index === selectors.length - 1) return selected.default
        return undefined
      }
      selected = selected?.[selector]
    }
    return selected
  }

  serializedError(error) {
    return {
      name: error?.name || 'Error',
      message: error?.message || String(error),
      stack: error?.stack,
      extraInfo: error?.extraInfo,
    }
  }

  async handle(message) {
    if (message?.event === CUSTOM_WORKER_REQUEST) return
    const startTime = Number(process.hrtime.bigint())
    try {
      for (const dependency of message.preludeDeps || workerData?.preludeDeps || []) require(dependency)
      const action = this.workFunction(message.workPath)
      if (typeof action !== 'function') throw new TypeError(`Worker action was not found at ${message.workPath}`)
      const returnVal = await action(message.workInput, this.cacheManager)
      this.port.postMessage({
        event: 'work done',
        id: message.id,
        returnVal,
        cacheSize: this.cacheManager.cacheItemSize(),
        startTime,
        endTime: Number(process.hrtime.bigint()),
      })
    } catch (error) {
      this.port.postMessage({
        event: 'work error',
        id: message.id,
        returnVal: this.serializedError(error),
        cacheSize: this.cacheManager.cacheItemSize(),
        startTime,
        endTime: Number(process.hrtime.bigint()),
      })
    }
  }
}

new HvigorWorkerRuntime(parentPort).start()
