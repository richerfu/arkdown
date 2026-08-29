'use strict'

const path = require('node:path')
const { Worker } = require('node:worker_threads')

// Rust owns worker admission, IDs, priority ordering, target filtering, and state
// transitions. This file only owns Worker objects, Promise resolvers, and
// plugin callbacks because those values cannot leave their V8 isolate.
function createHvigorWorkerRuntime(nativeRuntimeBinding, host, constants) {
  const state = new nativeRuntimeBinding.RuntimeHvigorWorkerPoolState(
    Number.isInteger(host._coreParameter.properties.hvigorPoolMaxSize)
      ? host._coreParameter.properties.hvigorPoolMaxSize
      : undefined,
  )
  const workerEntry = path.resolve(__dirname, 'hvigor-worker.js')
  const workers = new Map()
  const jobs = new Map()

  class WorkControl {
    constructor(submission, task, options) {
      this.workId = submission.workId
      this.task = task
      this.options = options
      this.submitTime = submission.submitTime
      this.customActions = new Map()
    }

    snapshot() {
      return state.snapshot(this.workId)
    }

    getId() {
      return this.workId
    }

    getWorkerId() {
      return this.snapshot().workerId
    }

    getSubmitTime() {
      return this.submitTime
    }

    getStartTime() {
      return this.snapshot().startTime
    }

    getEndTime() {
      return this.snapshot().endTime
    }

    getState() {
      return this.snapshot().state
    }

    getPriority() {
      return this.options.priority ?? constants.priority.MEDIUM
    }

    getCallback() {
      return this.options.callback
    }

    getCallbackInput() {
      return this.options.callbackInput ?? []
    }

    setPriority(priority) {
      if (!state.setWorkPriority(this.workId, Number(priority))) return false
      this.options.priority = Number(priority)
      return true
    }

    setCallback(callback, callbackInput) {
      if (!['waiting', 'running'].includes(this.getState())) return false
      this.options.callback = callback
      if (callbackInput !== undefined) this.options.callbackInput = callbackInput
      return true
    }

    getTaskPath() {
      const node = this.task.getNode()
      return node?.getName?.() ?? node?.getNodeName?.() ?? ''
    }

    getTaskName() {
      return this.task.getName()
    }

    getTaskCompletePath() {
      return this.task.getPath()
    }

    useReturnVal() {
      return Boolean(this.options.useReturnVal)
    }

    setCustomAction(customType, action) {
      if (!['waiting', 'running'].includes(this.getState())) return false
      this.customActions.set(String(customType), action)
      return true
    }

    popCustomAction(customType) {
      const key = String(customType)
      const action = this.customActions.get(key)
      this.customActions.delete(key)
      return action
    }
  }

  const workerError = (detail) => {
    const error = new Error(detail?.message || String(detail))
    error.name = detail?.name || 'Error'
    error.stack = detail?.stack || error.stack
    error.extraInfo = detail?.extraInfo
    return error
  }

  const settle = async (workerId, message, succeeded) => {
    const worker = workers.get(workerId)
    const workId = String(message.id ?? worker?.currentWorkId)
    const job = jobs.get(workId)
    if (!job) return
    state.finish(workId, succeeded, message.endTime ?? Number(process.hrtime.bigint()))
    worker.busy = false
    worker.currentWorkId = undefined
    jobs.delete(workId)
    try {
      if (!succeeded) throw workerError(message.returnVal || message.error)
      if (typeof job.options.callback === 'function') {
        const callbackInput = job.options.useReturnVal ? [message.returnVal] : job.options.callbackInput || []
        await job.options.callback(...callbackInput, workerId)
      }
      job.resolve(true)
    } catch (error) {
      job.reject(error)
    } finally {
      dispatch(workerId)
      provisionWorkers()
    }
  }

  const handleMessage = async (workerId, message) => {
    if (message?.event === constants.worker.customWorkerRequest) {
      const job = jobs.get(String(message.id))
      const action = job?.control.popCustomAction(message.customType)
      if (action) await action(message.param)
      workers.get(workerId)?.worker.postMessage({ ...message, done: true })
    } else if (message?.event === constants.worker.workDone) {
      await settle(workerId, message, true)
    } else if (message?.event === constants.worker.workError) {
      await settle(workerId, message, false)
    }
  }

  const ensureWorker = (workerId) => {
    if (workers.has(workerId)) return workers.get(workerId)
    const worker = new Worker(workerEntry, { env: process.env })
    const entry = { worker, busy: false, currentWorkId: undefined }
    workers.set(workerId, entry)
    worker.on('message', (message) => void handleMessage(workerId, message))
    worker.on('error', (error) => {
      if (entry.currentWorkId !== undefined) {
        void settle(workerId, { id: entry.currentWorkId, returnVal: error }, false)
      }
    })
    return entry
  }

  function dispatch(workerId) {
    const worker = workers.get(workerId)
    if (!worker || worker.busy) return false
    const workId = state.takeForWorker(workerId, Number(process.hrtime.bigint()))
    if (workId === undefined || workId === null) return false
    const job = jobs.get(workId)
    if (!job) return false
    worker.busy = true
    worker.currentWorkId = workId
    worker.worker.postMessage({
      id: workId,
      workPath: job.workPath,
      workInput: job.options.workInput,
      preludeDeps: job.options.preludeDeps,
    })
    return true
  }

  function provisionWorkers(candidates) {
    for (const [workerId, worker] of workers) {
      if (!worker.busy && dispatch(workerId)) return
    }
    if (!state.hasQueuedWork()) return
    const eligible = candidates || Array.from({ length: state.maxPoolNum }, (_value, index) => index)
    const workerId = eligible.find((candidate) => !workers.has(candidate))
    if (workerId !== undefined) {
      ensureWorker(workerId)
      dispatch(workerId)
    }
  }

  return {
    submit(task, workPath, rawOptions) {
      const options = rawOptions || {}
      const submission = state.submit(
        Boolean(host._coreParameter.startParams.parallelExecution),
        Number(options.priority ?? constants.priority.MEDIUM),
        Array.isArray(options.targetWorkers) ? options.targetWorkers.map(Number) : undefined,
      )
      const control = new WorkControl(submission, task, options)
      if (!submission.accepted) return control
      const completion = new Promise((resolve, reject) => {
        jobs.set(submission.workId, { control, workPath, options, resolve, reject })
      })
      task.pendingPromises.add(completion)
      provisionWorkers(submission.candidateWorkers)
      return control
    },

    async terminate() {
      state.terminate()
      const error = new Error('Hvigor worker pool terminated before the work completed')
      for (const job of jobs.values()) job.reject(error)
      jobs.clear()
      const active = [...workers.values()]
      workers.clear()
      await Promise.all(active.map(({ worker }) => worker.terminate().catch(() => 0)))
      return true
    },
  }
}

module.exports = { createHvigorWorkerRuntime }
