'use strict'

const assert = require('node:assert/strict')
const test = require('node:test')

const { ArkdownPackageCli } = require('../js/arkdown-cli.js')

test('package CLI parses native build options and explicit ArkTS 1.2', () => {
  const cli = new ArkdownPackageCli([], async () => ({ artifacts: [] }))
  const config = cli.parseBuildArgs([
    '--project',
    '/tmp/app',
    '--compile-plugin-path',
    './compile-a.ts',
    '--compile-plugin-path',
    './compile-b.mjs',
    '--arkts-version',
    '1.2',
    '--action',
    'device-test',
    '--analyze',
    'trace',
    '--check-entry',
    '--crossplatform',
    '--ignore-crossplatform-check',
    '--skip-checker',
  ])

  assert.deepEqual(config, {
    project: '/tmp/app',
    compilePluginPath: ['./compile-a.ts', './compile-b.mjs'],
    arktsVersion: '1.2',
    action: 'device-test',
    analyze: 'trace',
    checkEntry: true,
    isCrossplatform: true,
    ignoreCrossplatformCheck: true,
    skipChecker: true,
  })
})

test('package CLI validates typed reload and QuickFix actions', () => {
  const cli = new ArkdownPackageCli([], async () => ({ artifacts: [] }))

  assert.equal(cli.parseBuildArgs(['--action', 'hot-reload-build']).action, 'hot-reload-build')
  assert.equal(cli.parseBuildArgs(['--action', 'assemble-quickfix']).action, 'assemble-quickfix')
  assert.throws(
    () => cli.parseBuildArgs(['--action', 'CompileArkTS']),
    (error) => error.code === 'INVALID_ARKDOWN_CLI_OPTION',
  )
})

test('package CLI delegates the native build command', async () => {
  const calls = []
  const cli = new ArkdownPackageCli(['build', '--project', '/tmp/app'], async (config) => {
    calls.push(config)
    return { artifacts: [] }
  })

  assert.equal(await cli.run(), 0)
  assert.equal(typeof calls[0].onLog, 'function')
  assert.equal(calls[0].project, '/tmp/app')
})

test('package CLI delegates native check and clean commands', async () => {
  const calls = []
  const operations = {
    async check(config) {
      calls.push(['check', config])
    },
    async clean(config) {
      calls.push(['clean', config])
    },
  }

  assert.equal(
    await new ArkdownPackageCli(
      ['check', '--project', '/tmp/app', '--module', 'entry', '--crossplatform'],
      operations,
    ).run(),
    0,
  )
  assert.equal(await new ArkdownPackageCli(['clean', '--project', '/tmp/app'], operations).run(), 0)
  assert.deepEqual(calls, [
    ['check', { project: '/tmp/app', module: 'entry', isCrossplatform: true }],
    ['clean', { project: '/tmp/app' }],
  ])
})

test('package CLI rejects removed Hvigor task commands', async () => {
  const cli = new ArkdownPackageCli(['assembleHap'], async () => ({ artifacts: [] }))
  await assert.rejects(
    () => cli.run(),
    (error) => error.code === 'ARKDOWN_CLI_COMMAND_UNSUPPORTED',
  )
})

function captureOutput(tty = false, env = {}) {
  const output = { stdout: '', stderr: '' }
  return {
    output,
    streams: {
      stdout: {
        isTTY: tty,
        write: (line) => {
          output.stdout += line
        },
      },
      stderr: {
        write: (line) => {
          output.stderr += line
        },
      },
      env,
    },
  }
}

function stage(phase) {
  return {
    level: 'info',
    target: 'arkdown_build::coordinator',
    message: 'build step',
    fields: { source: 'rust_stage', phase, step: 3, total: 6, label: 'Bundling', elapsed_ms: 1234 },
  }
}

test('build logging options accept Vite and Rolldown spelling and reject invalid levels', () => {
  const cli = new ArkdownPackageCli([])
  for (const option of ['--logLevel', '--log-level', '-l']) {
    assert.equal(cli.parseBuildArgs([option, 'debug']).logLevel, 'debug')
  }
  assert.equal(cli.parseBuildArgs(['--logLevel=warn']).logLevel, 'warn')
  assert.deepEqual(cli.parseBuildArgs(['--silent', '--clearScreen', 'false']), {
    logLevel: 'silent',
    clearScreen: false,
  })
  assert.throws(() => cli.parseBuildArgs(['--logLevel', 'verbose']), { code: 'INVALID_ARKDOWN_CLI_OPTION' })
  assert.throws(() => cli.parseBuildArgs(['--clearScreen', 'maybe']), { code: 'INVALID_ARKDOWN_CLI_OPTION' })
})

test('non-terminal build output shows real stages, diagnostics, artifacts and total time', async () => {
  const { output, streams } = captureOutput()
  const cli = new ArkdownPackageCli(
    ['build'],
    async ({ onLog }) => {
      onLog(stage('start'))
      onLog({ level: 'warn', message: 'check warning', fields: { diagnostic: 'source.ts:4', module: 'entry' } })
      onLog(stage('end'))
      return { artifacts: ['entry.hap'], elapsedMs: 2045 }
    },
    streams,
  )
  assert.equal(await cli.run(), 0)
  assert.match(output.stdout, /\[3\/6\] Bundling\.\.\.\n/)
  assert.match(output.stdout, /✓ \[3\/6\] Bundling \(1\.23s\)/)
  assert.match(output.stdout, /entry\.hap\n✓ built in 2\.04s\n/)
  assert.match(output.stderr, /check warning.*diagnostic=source\.ts:4.*module=entry/)
  assert.equal((output.stdout + output.stderr).includes('\x1b'), false)
  assert.equal((output.stdout + output.stderr).includes('\r'), false)
})

test('progress updates in place only on a terminal outside CI', async () => {
  for (const [args, env, dynamic] of [
    [[], {}, true],
    [[], { CI: 'true' }, false],
    [['--clearScreen', 'false'], {}, false],
  ]) {
    const { output, streams } = captureOutput(true, env)
    await new ArkdownPackageCli(
      ['build', ...args],
      async (config) => {
        assert.equal(config.clearScreen, undefined, 'terminal settings stay in JavaScript')
        config.onLog(stage('start'))
        config.onLog({ level: 'warn', message: 'warning' })
        config.onLog(stage('end'))
        return { artifacts: [], elapsedMs: 1 }
      },
      streams,
    ).run()
    assert.equal(output.stdout.includes('\r\x1b[2K'), dynamic)
    assert.match(output.stderr, /warning/)
  }
})

test('logLevel filters progress and diagnostics while silent failures still exit with one', async () => {
  for (const [level, expected] of [
    ['info', ['info', 'warn', 'error']],
    ['warn', ['warn', 'error']],
    ['error', ['error']],
    ['debug', ['debug', 'info', 'warn', 'error']],
    ['silent', []],
  ]) {
    const { output, streams } = captureOutput()
    const code = await new ArkdownPackageCli(
      ['build', '--logLevel', level],
      async ({ onLog }) => {
        for (const entry of ['debug', 'info', 'warn', 'error']) onLog({ level: entry, message: `event-${entry}` })
        throw new Error('broken package')
      },
      streams,
    ).run()
    assert.equal(code, 1)
    const text = output.stdout + output.stderr
    for (const entry of ['debug', 'info', 'warn', 'error'])
      assert.equal(text.includes(`event-${entry}`), expected.includes(entry))
    if (level === 'silent') assert.equal(text, '')
    else assert.match(output.stderr, /Build failed.*\nerror: broken package/)
  }
})

test('watch builds report artifact completion before the long-lived service resolves', async () => {
  const { output, streams } = captureOutput()
  await new ArkdownPackageCli(
    ['build', '--watch'],
    async ({ onLog }) => {
      onLog({ level: 'info', message: 'build succeeded', fields: { artifact: 'entry.hap', elapsed_seconds: 1.5 } })
      assert.match(output.stdout, /entry\.hap\n✓ built in 1\.50s/)
      return { artifacts: [], elapsedMs: 1500 }
    },
    streams,
  ).run()
})
