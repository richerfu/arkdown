'use strict'

const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')
const { build } = require('..')

const enabled = process.env.ARKDOWN_NATIVE_LOG_E2E === '1'

test(
  'real native build flushes ordered progress and plugin logs before resolving',
  { skip: !enabled, timeout: 120000 },
  async (context) => {
    const project = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-native-log-'))
    context.after(() => fs.rmSync(project, { recursive: true, force: true }))
    fs.cpSync(path.resolve(__dirname, '../../../tests/fixtures/minimal-hap'), project, {
      recursive: true,
      filter: (source) => !['build', '.hvigor'].includes(path.basename(source)),
    })
    fs.writeFileSync(
      path.join(project, 'oh-package.json5'),
      JSON.stringify({ modelVersion: '6.0.0', dependencies: {}, devDependencies: {} }),
    )
    const logs = []
    const result = await build(
      { project, cache: false, logLevel: 'debug', onLog: (log) => logs.push(log) },
      {
        name: 'logging-fixture',
        beforeBuild() {
          this.debug('plugin debug is enabled')
          this.share.getLogger('logging-fixture').warn('plugin warning is delivered')
        },
      },
    )
    assert.ok(result.artifacts.some((artifact) => artifact.endsWith('.hap')))
    assert.ok(logs.some((log) => log.message === 'plugin debug is enabled'))
    assert.ok(logs.some((log) => log.message === 'plugin warning is delivered'))
    const stages = logs.filter((log) => log.fields.source === 'rust_stage')
    assert.deepEqual(
      stages.map((log) => [log.fields.step, log.fields.phase]),
      [1, 2, 3, 4, 5, 6].flatMap((step) => [
        [step, 'start'],
        [step, 'end'],
      ]),
    )
    for (const log of stages.filter((log) => log.fields.phase === 'end')) assert.ok(log.fields.elapsed_ms >= 0)
    assert.equal(
      logs.some((log) => log.target === 'arkdown::log_flush'),
      false,
    )
    assert.equal(fs.existsSync(path.join(project, '.hvigor/outputs/build-logs/build.log')), false)

    const cli = path.resolve(__dirname, '../js/arkdown-cli.js')
    const run = (args, colorEnv = {}) =>
      spawnSync(process.execPath, ['--expose-gc', cli, 'build', '--project', project, ...args], {
        encoding: 'utf8',
        timeout: 120000,
        env: { ...process.env, NO_COLOR: '', FORCE_COLOR: '0', ...colorEnv },
      })
    const output = run([])
    assert.equal(output.status, 0, output.stdout + output.stderr)
    assert.match(output.stdout, /\[1\/6\] Preparing project/)
    assert.match(output.stdout, /\[6\/6\] Packaging/)
    assert.match(output.stdout, /\.hap\n✓ built in /)
    assert.equal((output.stdout + output.stderr).includes('\x1b'), false)
    assert.equal((output.stdout + output.stderr).includes('\r'), false)
    const colored = run([], { FORCE_COLOR: '1' })
    assert.equal(colored.status, 0, colored.stderr)
    assert.ok(colored.stdout.includes('\x1b[36m[1/6] Preparing project...\x1b[39m'))
    assert.ok(colored.stdout.includes('\x1b[32m✓ built in '))
    assert.equal(colored.stdout.includes('\r'), false, 'forced color does not animate redirected progress')
    const disabled = run([], { FORCE_COLOR: undefined, NO_COLOR: '1' })
    assert.equal(disabled.status, 0, disabled.stderr)
    assert.equal((disabled.stdout + disabled.stderr).includes('\x1b'), false)
    const quiet = run(['--logLevel', 'silent'], { FORCE_COLOR: '1' })
    assert.equal(quiet.status, 0, quiet.stderr)
    assert.equal(quiet.stdout + quiet.stderr, '')
    const failed = run(['--silent', '--module', 'missing-module'])
    assert.equal(failed.status, 1)
    assert.equal(failed.stdout + failed.stderr, '')
  },
)

test(
  'native subscribers isolate concurrent failed requests and release callbacks',
  { skip: !enabled, timeout: 10000 },
  async (context) => {
    const project = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-native-log-error-'))
    context.after(() => fs.rmSync(project, { recursive: true, force: true }))
    const visible = []
    const hidden = []
    const results = await Promise.allSettled([
      build({ project, logLevel: 'debug', onLog: (log) => visible.push(log) }),
      build({ project, logLevel: 'silent', onLog: (log) => hidden.push(log) }),
    ])
    assert.deepEqual(
      results.map((result) => result.status),
      ['rejected', 'rejected'],
    )
    assert.ok(visible.some((log) => log.fields.source === 'rust_stage'))
    assert.deepEqual(hidden, [])
    await assert.rejects(
      build({
        project,
        onLog() {
          throw new Error('log callback failed')
        },
      }),
    )
    const later = []
    await assert.rejects(build({ project, logLevel: 'info', onLog: (log) => later.push(log) }))
    assert.ok(later.some((log) => log.fields.source === 'rust_stage'))
  },
)
