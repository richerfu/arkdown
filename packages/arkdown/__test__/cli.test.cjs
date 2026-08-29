'use strict'

const assert = require('node:assert/strict')
const test = require('node:test')

const { ArkdownPackageCli } = require('../js/arkdown-cli.js')

test('package CLI parses repeated hvigor plugin path flags', () => {
  const cli = new ArkdownPackageCli([], async () => ({ artifacts: [] }))

  const config = cli.parseBuildArgs([
    '--project',
    '/tmp/app',
    '--compile-plugin-path',
    './compile-a.js',
    '--compile-plugin-path',
    './compile-b.js',
    '--hvigor-plugin-file',
    './hvigor-plugin.js',
    '--build-version',
    '2026.07.23',
    '--enable-zig',
  ])

  assert.deepEqual(config, {
    project: '/tmp/app',
    compilePluginPath: ['./compile-a.js', './compile-b.js'],
    hvigorPluginFile: './hvigor-plugin.js',
    buildVersion: '2026.07.23',
    enableZig: true,
  })
})

test('hvigor-compatible CLI keeps execution mode separate from module and build mode', () => {
  const cli = new ArkdownPackageCli([])

  const config = cli.parseHvigorArgs([
    'assembleHap',
    '--mode',
    'module',
    '-p',
    'module=entry@phone',
    '-p',
    'buildMode=release',
    '-p',
    'product=tablet',
    '--no-daemon',
    '--incremental',
  ])

  assert.deepEqual(config, {
    tasks: ['assembleHap'],
    prop: ['module=entry@phone', 'buildMode=release', 'product=tablet'],
    executionMode: 'module',
    module: 'entry@phone',
    mode: 'release',
    product: 'tablet',
    daemon: false,
    cache: true,
  })
})

test('hvigor-compatible CLI delegates raw task names and properties to one command invocation', async () => {
  const calls = []
  const cli = new ArkdownPackageCli([], undefined, async (config) => {
    calls.push(config)
    return { artifacts: [], elapsedMs: 1, output: [] }
  })
  cli.argv = ['--mode', 'project', 'tasks', '-p', 'buildVersion=1.2.3']

  assert.equal(await cli.run(), 0)
  assert.deepEqual(calls, [
    {
      tasks: ['tasks'],
      prop: ['buildVersion=1.2.3'],
      executionMode: 'project',
      buildVersion: '1.2.3',
    },
  ])
})

test('hvigor-compatible CLI applies the latest buildVersion grammar', () => {
  const cli = new ArkdownPackageCli([])

  assert.throws(
    () => cli.parseHvigorArgs(['assembleHap', '-p', 'buildVersion=01.2']),
    (error) => error.code === 'INVALID_BUILDVERSION_PARAMETER',
  )
  assert.doesNotThrow(() => cli.parseHvigorArgs(['assembleHap', '-p', 'buildVersion=2026.8.1']))
})

test('hvigor-compatible daemon commands preserve latest CLI exit behavior without a fake daemon', () => {
  const cli = new ArkdownPackageCli([])

  assert.deepEqual(cli.daemonCommandOutput(cli.parseHvigorArgs(['--status-daemon'])), ['PID\tSTATUS\tPORT\tROOT_PATH'])
  assert.deepEqual(cli.daemonCommandOutput(cli.parseHvigorArgs(['--stop-daemon'])), ['Stopped 0 daemon in total.'])
  assert.deepEqual(cli.daemonCommandOutput(cli.parseHvigorArgs(['--stop-daemon-all'])), ['Stopped 0 daemon in total.'])
  assert.deepEqual(cli.daemonCommandOutput(cli.parseHvigorArgs(['--stop-daemon', '--status-daemon'])), [
    'Stopped 0 daemon in total.',
  ])
})
