#!/usr/bin/env node

import { execFile, execFileSync, spawn } from 'node:child_process'
import { once } from 'node:events'
import { existsSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { promisify } from 'node:util'

const execFileAsync = promisify(execFile)
const scriptDirectory = dirname(fileURLToPath(import.meta.url))
const argv = process.argv.slice(2)
const separator = argv.indexOf('--')

if (separator === -1 || separator === argv.length - 1) {
  process.stderr.write('Usage: measure-process-tree.mjs [--interval-ms <milliseconds>] -- <command> [args...]\n')
  process.exit(2)
}

let intervalMs = 100
for (let index = 0; index < separator; index += 1) {
  if (argv[index] !== '--interval-ms' || index + 1 >= separator) {
    process.stderr.write(`Unknown option: ${argv[index]}\n`)
    process.exit(2)
  }
  intervalMs = Number(argv[index + 1])
  index += 1
}

if (!Number.isInteger(intervalMs) || intervalMs <= 0) {
  process.stderr.write('--interval-ms must be a positive integer\n')
  process.exit(2)
}

const [command, ...commandArgs] = argv.slice(separator + 1)
let darwinSampler
if (process.platform === 'darwin') {
  const source = join(scriptDirectory, 'process-memory-darwin.c')
  darwinSampler = join(tmpdir(), `arkdown-process-memory-darwin-${process.getuid()}`)
  if (!existsSync(darwinSampler) || statSync(darwinSampler).mtimeMs < statSync(source).mtimeMs) {
    execFileSync('cc', [source, '-O2', '-o', darwinSampler], { stdio: 'inherit' })
  }
}

const startedAt = process.hrtime.bigint()
const child = spawn(command, commandArgs, {
  cwd: process.cwd(),
  env: process.env,
  stdio: 'inherit',
})

await once(child, 'spawn')

const trackedPids = new Set([child.pid])
let sampleCount = 0
let peakTreeRssKiB = 0
let peakSingleRssKiB = 0
let peakSingleProcess
let peakTreeRssProcesses = []
let peakTreeFootprintKiB = 0
let peakSingleFootprintKiB = 0
let peakSingleFootprintProcess
let peakTreeFootprintProcesses = []
let sampleError
let stopped = false
let timer
let activeSample = Promise.resolve()

const sample = async () => {
  try {
    const { stdout } = darwinSampler
      ? await execFileAsync(darwinSampler, [...trackedPids].map(String), {
          encoding: 'utf8',
          maxBuffer: 64 * 1024 * 1024,
        })
      : await execFileAsync('ps', ['-axo', 'pid=,ppid=,rss=,command='], {
          encoding: 'utf8',
          maxBuffer: 64 * 1024 * 1024,
        })
    const processes = []
    const childrenByParent = new Map()
    for (const line of stdout.split('\n')) {
      let processInfo
      if (darwinSampler) {
        const fields = line.split('\t')
        if (fields.length < 6) continue
        processInfo = {
          pid: Number(fields[0]),
          ppid: Number(fields[1]),
          rssKiB: Number(fields[2]) / 1024,
          footprintKiB: Number(fields[3]) / 1024,
          lifetimeMaxFootprintKiB: Number(fields[4]) / 1024,
          command: fields.slice(5).join('\t'),
        }
      } else {
        const match = line.match(/^\s*(\d+)\s+(\d+)\s+(\d+)\s+(.*)$/)
        if (!match) continue
        processInfo = {
          pid: Number(match[1]),
          ppid: Number(match[2]),
          rssKiB: Number(match[3]),
          footprintKiB: Number(match[3]),
          lifetimeMaxFootprintKiB: Number(match[3]),
          command: match[4],
        }
      }
      processes.push(processInfo)
      const children = childrenByParent.get(processInfo.ppid)
      if (children) {
        children.push(processInfo.pid)
      } else {
        childrenByParent.set(processInfo.ppid, [processInfo.pid])
      }
    }

    const pending = [...trackedPids]
    for (let index = 0; index < pending.length; index += 1) {
      const descendants = childrenByParent.get(pending[index]) ?? []
      for (const pid of descendants) {
        if (trackedPids.has(pid)) continue
        trackedPids.add(pid)
        pending.push(pid)
      }
    }

    const treeProcesses = processes.filter((processInfo) => trackedPids.has(processInfo.pid))
    const treeRssKiB = treeProcesses.reduce((total, processInfo) => total + processInfo.rssKiB, 0)
    const treeFootprintKiB = treeProcesses.reduce((total, processInfo) => total + processInfo.footprintKiB, 0)
    sampleCount += 1

    for (const processInfo of treeProcesses) {
      if (processInfo.rssKiB > peakSingleRssKiB) {
        peakSingleRssKiB = processInfo.rssKiB
        peakSingleProcess = processInfo
      }
      if (processInfo.footprintKiB > peakSingleFootprintKiB) {
        peakSingleFootprintKiB = processInfo.footprintKiB
        peakSingleFootprintProcess = processInfo
      }
    }

    if (treeRssKiB > peakTreeRssKiB) {
      peakTreeRssKiB = treeRssKiB
      peakTreeRssProcesses = treeProcesses.toSorted((left, right) => right.rssKiB - left.rssKiB).map(summarizeProcess)
    }

    if (treeFootprintKiB > peakTreeFootprintKiB) {
      peakTreeFootprintKiB = treeFootprintKiB
      peakTreeFootprintProcesses = treeProcesses
        .toSorted((left, right) => right.footprintKiB - left.footprintKiB)
        .map(summarizeProcess)
    }
  } catch (error) {
    sampleError = error instanceof Error ? error.message : String(error)
  }
}

const summarizeProcess = (processInfo) => {
  return {
    ...processInfo,
    command: processInfo.command.length > 240 ? `${processInfo.command.slice(0, 237)}...` : processInfo.command,
  }
}

const scheduleSample = () => {
  if (stopped) return
  activeSample = sample().finally(() => {
    if (!stopped) timer = setTimeout(scheduleSample, intervalMs)
  })
}

const forwardSignal = (signal) => {
  if (child.exitCode === null && child.signalCode === null) child.kill(signal)
}

process.once('SIGINT', () => forwardSignal('SIGINT'))
process.once('SIGTERM', () => forwardSignal('SIGTERM'))

scheduleSample()
const [exitCode, signal] = await once(child, 'exit')
stopped = true
clearTimeout(timer)
await activeSample
await sample()

const elapsedSeconds = Number(process.hrtime.bigint() - startedAt) / 1_000_000_000
const result = {
  command: [command, ...commandArgs],
  cwd: process.cwd(),
  intervalMs,
  elapsedSeconds,
  exitCode,
  signal,
  sampleCount,
  peakSingleRssKiB,
  peakSingleRssGiB: peakSingleRssKiB / 1024 / 1024,
  peakSingleProcess,
  peakTreeRssKiB,
  peakTreeRssGiB: peakTreeRssKiB / 1024 / 1024,
  peakTreeRssProcesses,
  peakSingleFootprintKiB,
  peakSingleFootprintGiB: peakSingleFootprintKiB / 1024 / 1024,
  peakSingleFootprintProcess,
  peakTreeFootprintKiB,
  peakTreeFootprintGiB: peakTreeFootprintKiB / 1024 / 1024,
  peakTreeFootprintProcesses,
  sampleError,
}

process.stdout.write(`\nARKDOWN_PROCESS_TREE_MEASUREMENT=${JSON.stringify(result, null, 2)}\n`)
process.exitCode = exitCode ?? (signal ? 1 : 0)
