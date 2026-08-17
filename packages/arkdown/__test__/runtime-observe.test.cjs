'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')

const { HvigorRuntimeHost } = require('../runtime')
const native = require('../index.js')

function tempProject() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-observe-'))
}

test('installObservability writes one JS info line into build.log and not a second log4js file handle', () => {
  const root = tempProject()
  const host = new HvigorRuntimeHost(root)
  host.installObservability()
  const hvigor = host._makeHvigorMock()
  const logger = hvigor.HvigorLogger.getLogger('fixture')
  logger.info('observe-info-line')
  host.finishObservability()
  const logFile = path.join(root, '.hvigor', 'outputs', 'build-logs', 'build.log')
  assert.equal(fs.existsSync(logFile), true)
  const content = fs.readFileSync(logFile, 'utf8')
  assert.match(content, /observe-info-line/)
  assert.match(content, /arkdown_napi::hvigor/)
  assert.equal(hvigor.getConfiguration().appenders['debug-log-file'].type, 'file')
  assert.equal(
    hvigor
      .getConfiguration()
      .appenders['debug-log-file'].filename.endsWith(path.join('.hvigor', 'outputs', 'build-logs', 'build.log')),
    true,
  )
})

test('runtime observability NAPI exports match the generated camelCase names', () => {
  assert.equal(typeof native.installRuntimeObservability, 'function')
  assert.equal(typeof native.emitRuntimeLog, 'function')
  assert.equal(typeof native.runtimeObservabilityIsEnabled, 'function')
  assert.equal(typeof native.finishRuntimeObservability, 'function')
  assert.equal(typeof native.persistRuntimeReport, 'function')
  assert.equal(typeof native.persistRuntimeDetails, 'function')
  assert.equal(native.persistRuntimeMonitor, undefined)
  assert.equal(typeof native.RuntimeProcessMonitor, 'function')
  assert.equal(typeof native.RuntimeTaskCacheSession, 'function')
  assert.equal(typeof native.RuntimeFileSnapshot, 'function')
  assert.equal(typeof native.RuntimeTaskSnapshot, 'function')
  assert.equal(native.beginRuntimeInvocation, undefined)
  assert.equal(native.finishRuntimeInvocation, undefined)
  const session = new native.RuntimeBuildScriptSession(tempProject(), undefined, {}, [])
  assert.equal(typeof session.beginInvocation, 'function')
  assert.equal(typeof session.finishInvocation, 'function')
})

test('Rust process monitor owns Hvigor sampling state and report persistence', async () => {
  const root = tempProject()
  const cacheDir = path.join(root, '.hvigor')
  native.installRuntimeObservability({
    mode: 'napi',
    cacheDir,
    buildId: '202608151234567890',
    logLevel: 'info',
    consoleOwner: 'log4js',
  })
  const sampler = () => ({
    rss: 10 * 1024 * 1024,
    heapTotal: 8 * 1024 * 1024,
    heapUsed: 6 * 1024 * 1024,
    external: 4 * 1024 * 1024,
    arrayBuffers: 2 * 1024 * 1024,
  })
  const monitor = new native.RuntimeProcessMonitor(sampler, process.pid)
  monitor.start()
  await new Promise((resolve) => setTimeout(resolve, 1100))
  const monitorPath = monitor.stop()
  assert.equal(monitorPath, path.join(cacheDir, 'report', 'report-monitor-202608151234567890.json'))
  const report = JSON.parse(fs.readFileSync(monitorPath, 'utf8'))
  assert.equal(report.version, '1.0')
  assert.equal(report.pid, process.pid)
  assert.equal(report.data.length, 1)
  assert.deepEqual(
    {
      rss: report.data[0].rss,
      heapTotal: report.data[0].heapTotal,
      heapUsed: report.data[0].heapUsed,
      external: report.data[0].external,
      arrayBuffers: report.data[0].arrayBuffers,
    },
    { rss: 10, heapTotal: 8, heapUsed: 6, external: 4, arrayBuffers: 2 },
  )
  assert.equal(Number.isFinite(report.data[0].time), true)
})

test('process-wide Hvigor JavaScript runtime rejects overlapping builds and releases deterministically', () => {
  const firstRoot = tempProject()
  const secondRoot = tempProject()
  const first = new native.RuntimeBuildScriptSession(firstRoot, undefined, {}, [])
  const second = new native.RuntimeBuildScriptSession(secondRoot, undefined, {}, [])
  const token = first.beginInvocation()
  assert.throws(() => second.beginInvocation(), /ARKDOWN_CONCURRENT_RUNTIME_INVOCATION/)
  assert.throws(() => first.finishInvocation('wrong-token'), /TOKEN_MISMATCH/)
  first.finishInvocation(token)
  const nextToken = second.beginInvocation()
  second.finishInvocation(nextToken)
})

test('report and trace details persistence use the Rust observability owner', () => {
  const root = tempProject()
  const cacheDir = path.join(root, '.hvigor')
  native.installRuntimeObservability({
    mode: 'napi',
    cacheDir,
    buildId: '202608141234567890',
    logLevel: 'info',
    consoleOwner: 'log4js',
  })
  const reportPath = native.persistRuntimeReport(JSON.stringify({ version: '2.0', events: [] }), false)
  const detailsPath = native.persistRuntimeDetails(JSON.stringify({ phase: 'fixture' }))
  assert.equal(reportPath, path.join(cacheDir, 'report', 'report-202608141234567890.json'))
  assert.equal(detailsPath, path.join(cacheDir, 'outputs', 'logs', 'details', 'details.json'))
  assert.deepEqual(JSON.parse(fs.readFileSync(detailsPath, 'utf8')), { phase: 'fixture' })
})

test('re-install updates the file filter instead of freezing the first EnvFilter', () => {
  const root = tempProject()
  native.installRuntimeObservability({
    mode: 'napi',
    cacheDir: path.join(root, '.hvigor'),
    buildId: '111111111111111111',
    logLevel: 'error',
    consoleOwner: 'log4js',
  })
  native.emitRuntimeLog({
    level: 'info',
    category: 'fixture',
    message: 'should-be-filtered',
    source: 'js_plugin',
  })
  native.finishRuntimeObservability()
  native.installRuntimeObservability({
    mode: 'napi',
    cacheDir: path.join(root, '.hvigor'),
    buildId: '111111111111111111',
    logLevel: 'debug',
    consoleOwner: 'log4js',
  })
  native.emitRuntimeLog({
    level: 'info',
    category: 'fixture',
    message: 'should-be-visible',
    source: 'js_plugin',
  })
  native.finishRuntimeObservability()
  const logFile = path.join(root, '.hvigor', 'outputs', 'build-logs', 'build.log')
  const content = fs.existsSync(logFile) ? fs.readFileSync(logFile, 'utf8') : ''
  assert.doesNotMatch(content, /should-be-filtered/)
  assert.match(content, /should-be-visible/)
})
