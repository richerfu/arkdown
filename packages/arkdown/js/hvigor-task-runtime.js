'use strict'

// Task classes remain in V8 because user tasks subclass them and attach arbitrary
// callbacks. Ordered file inventories, task metadata, hook scheduling, and
// execution state are owned by the native objects referenced below.
function createHvigorTaskUtilities(nativeRuntimeBinding, options = {}) {
  class FileSet {
    constructor() {
      this.nativeFileSet = new nativeRuntimeBinding.RuntimeHvigorFileSet()
      this.optionsByToken = new Map()
    }

    addEntry(entryPath, entryOptions) {
      const mutation = this.nativeFileSet.addEntry(String(entryPath))
      if (mutation.replacedToken !== undefined) this.optionsByToken.delete(mutation.replacedToken)
      this.optionsByToken.set(mutation.token, entryOptions || {})
      return this
    }

    addEntries(entries, entryOptions) {
      for (const mutation of this.nativeFileSet.addEntries(entries.map(String))) {
        if (mutation.replacedToken !== undefined) this.optionsByToken.delete(mutation.replacedToken)
        this.optionsByToken.set(mutation.token, { ...entryOptions })
      }
      return this
    }

    deleteEntry(entryPath) {
      const token = this.nativeFileSet.deleteEntry(String(entryPath))
      if (token !== undefined) this.optionsByToken.delete(token)
      return this
    }

    collect() {
      return new Map(
        this.nativeFileSet
          .collectEntries()
          .map(({ path: entryPath, token }) => [entryPath, this.optionsByToken.get(token)]),
      )
    }
  }

  const nodeName = (node) => node?.getName?.() ?? node?.getNodeName?.() ?? ''
  const isProjectNode = (node) => node?.classKind === 'project'
  const taskPath = (node, name) => `${isProjectNode(node) ? '' : nodeName(node)}:${String(name)}`

  class CoreTask {}

  class TaskView {
    constructor(task) {
      this.task = task
    }

    getDependencies() {
      return this.task
        .getDependsOn()
        .map((dependency) => (dependency.startsWith(':') ? dependency.slice(1) : dependency))
    }

    getDependenciesInternal() {
      return this.getDependencies()
    }

    getName() {
      return this.task.getName()
    }

    getNameInternal() {
      return this.getName()
    }

    setEnable(enabled) {
      this.task.setEnabled(enabled)
    }

    setEnableInternal(enabled) {
      this.setEnable(enabled)
    }

    beforeRun(callback) {
      this.task.beforeRun(callback)
    }

    beforeRunInternal(callback) {
      this.beforeRun(callback)
    }

    afterRun(callback) {
      this.task.afterRun(callback)
    }

    afterRunInternal(callback) {
      this.afterRun(callback)
    }
  }

  class DefaultTask extends CoreTask {
    constructor(node, taskInfo) {
      super()
      const details = typeof taskInfo === 'string' ? { name: taskInfo || 'default' } : taskInfo || {}
      const normalized = { group: 'Other', isEnabled: true, ...details }
      this.node = node
      this.nativeTaskState = new nativeRuntimeBinding.RuntimeHvigorTaskState(
        String(nodeName(node)),
        isProjectNode(node),
        {
          name: normalized.name,
          group: normalized.group,
          isEnabled: normalized.isEnabled,
          description: normalized.description ?? '',
          sourcePluginId: normalized.sourcePluginId ?? '',
        },
      )
      this.nativeTaskHooks = new nativeRuntimeBinding.RuntimeHvigorTaskHooks()
      this.taskExecutedStatus = new nativeRuntimeBinding.RuntimeHvigorTaskExecutionState()
      this.beforeHookRefs = []
      this.afterHookRefs = []
      const pending = new Set()
      this.pendingPromises = {
        add: (promise) => pending.add(promise),
        get: () => [...pending],
        clear: () => pending.clear(),
      }
      this.taskLog = options.getLogger?.(this.getPath())
    }

    getWorkerPool() {
      return options.getWorkerPool?.()
    }

    async execute() {
      await this.executeBeforeRun()
      await this.getAction().call(this)
    }

    taskShouldDo() {
      return true
    }

    // Hvigor's base task intentionally leaves failure handling to subclasses.
    onFailed(_error) {}

    setDependsOn(...dependencies) {
      this.nativeTaskState.setDependsOn(dependencies.map(String))
      return this
    }

    getDependsOn() {
      return this.nativeTaskState.getDependsOn()
    }

    addDependsOn(dependency) {
      this.nativeTaskState.addDependsOn(String(dependency))
      return this
    }

    dependsOn(dependency, target) {
      const dependencyName = dependency instanceof CoreTask ? dependency.getName() : dependency
      const targetNode =
        target === undefined ? this.node : typeof target === 'string' ? this.findTargetNode(target) : target
      if (targetNode === undefined) {
        throw new Error(`Cannot find node ${String(target)} while setting dependency for ${String(this.getName())}`)
      }
      this.nativeTaskState.addDependsOn(taskPath(targetNode, dependencyName))
      return this
    }

    findTargetNode(name) {
      return this.node.findModuleByName?.(name) ?? (this.isProject(name) ? this.node.getProject?.() : undefined)
    }

    isProject(name) {
      return name === ':' || name === this.node.getProject?.()?.getName?.()
    }

    // This is the observed abstract-default action from Hvigor 6.24.3.
    registryAction() {
      return () => {}
    }

    getAction() {
      return this.registryAction()
    }

    getNode() {
      return this.node
    }

    getName() {
      return this.nativeTaskState.getName()
    }

    getPath() {
      return this.nativeTaskState.getPath()
    }

    getTaskLog() {
      return this.taskLog
    }

    getEnabled() {
      return this.nativeTaskState.getEnabled()
    }

    setEnabled(enabled) {
      this.nativeTaskState.setEnabled(enabled)
      return this
    }

    setDescription(description) {
      this.nativeTaskState.setDescription(description)
      return this
    }

    getDescription() {
      return this.nativeTaskState.getDescription()
    }

    getGroup() {
      return this.nativeTaskState.getGroup()
    }

    setGroup(group) {
      this.nativeTaskState.setGroup(group)
      return this
    }

    getSourcePluginId() {
      return this.nativeTaskState.getSourcePluginId()
    }

    doNotTrackState(reason) {
      this.taskExecutedStatus.unTrackStateReason = reason
    }

    getHvigorTask() {
      return new TaskView(this)
    }

    beforeRun(callback) {
      const bridge = () => callback.call(this)
      this.beforeHookRefs.push({ callback, bridge })
      this.nativeTaskHooks.beforeRun(bridge)
    }

    afterRun(callback) {
      const bridge = () => callback.call(this)
      this.afterHookRefs.push({ callback, bridge })
      this.nativeTaskHooks.afterRun(bridge)
    }

    getBeforeRunStack() {
      const owner = this
      return {
        get stack() {
          return owner.beforeHookRefs.map(({ callback }) => callback)
        },
        size: () => owner.beforeHookRefs.length,
        isEmpty: () => owner.beforeHookRefs.length === 0,
        peek: () => owner.beforeHookRefs.at(-1)?.callback,
      }
    }

    getAfterRunQueue() {
      const owner = this
      return {
        get elements() {
          return owner.afterHookRefs.map(({ callback }) => callback)
        },
        size: () => owner.afterHookRefs.length,
        peek: () => owner.afterHookRefs[0]?.callback,
      }
    }

    async executeBeforeRun() {
      try {
        await this.nativeTaskHooks.executeBefore()
      } finally {
        this.beforeHookRefs = []
      }
    }

    async executeAfterRun() {
      try {
        await this.nativeTaskHooks.executeAfter()
      } finally {
        this.afterHookRefs = []
      }
    }
  }

  class DefaultSyncTask extends DefaultTask {
    constructor(node, taskInfo) {
      super(node, taskInfo)
      this.setGroup('Sync')
    }
  }

  class IncrementalTask extends DefaultTask {
    declareInputs() {
      return new Map()
    }

    declareInputFiles() {
      return new FileSet()
    }

    declareOutputFiles() {
      return new FileSet()
    }

    async beforeAlwaysAction() {}
  }

  class IncrementalExecTask extends IncrementalTask {
    declareExecutionTool() {
      return ''
    }

    declareExecutionCommand() {
      return ''
    }

    declareExecutionEnv() {
      return new Map()
    }
  }

  class TaskInput {
    constructor(declareInputs, declareInputFiles) {
      this.declareInputs = declareInputs
      this.declareInputFiles = declareInputFiles
    }

    file(entryPath) {
      this.declareInputFiles.addEntry(entryPath)
      return this
    }

    files(entries) {
      this.declareInputFiles.addEntries(entries)
      return this
    }

    property(name, value) {
      this.declareInputs.set(name, value)
      return this
    }
  }

  class TaskOutput {
    constructor(declareOutputFiles) {
      this.declareOutputFiles = declareOutputFiles
    }

    file(entryPath) {
      this.declareOutputFiles.addEntry(entryPath)
      return this
    }

    files(entries) {
      this.declareOutputFiles.addEntries(entries)
      return this
    }
  }

  for (const prototype of [TaskInput.prototype, TaskOutput.prototype]) {
    for (const method of ['file', 'files', 'property']) {
      if (typeof prototype[method] === 'function') prototype[`${method}Internal`] = prototype[method]
    }
  }

  const declaratorContainer = Symbol('ARKDOWN_HVIGOR_DECLARATOR_CONTAINER')
  const registerDeclarator = (target, record) => {
    if (!Object.hasOwn(target, declaratorContainer)) {
      Object.defineProperty(target, declaratorContainer, { value: [], enumerable: false })
    }
    target[declaratorContainer].push(record)
  }
  const baseDecorator = (declaratorType) => () => (target, propertyKey) =>
    registerDeclarator(target, { propertyKey, declaratorType })
  const fileDecorator = (declaratorType) => (entryOptions) => (target, propertyKey) =>
    registerDeclarator(target, { propertyKey, declaratorType, options: entryOptions })

  return {
    CoreTask,
    DefaultTask,
    DefaultSyncTask,
    IncrementalExecTask,
    FileSet,
    TaskInput,
    TaskOutput,
    Input: baseDecorator('Input'),
    Inputs: baseDecorator('Inputs'),
    InputFile: fileDecorator('InputFile'),
    InputFiles: fileDecorator('InputFiles'),
    OutputFile: fileDecorator('OutputFile'),
    OutputFiles: fileDecorator('OutputFiles'),
    IgnoreSuperIncremental: baseDecorator('IgnoreSuperIncremental'),
  }
}

module.exports = { createHvigorTaskUtilities }
