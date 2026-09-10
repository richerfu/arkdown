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
  assert.deepEqual(calls, [{ project: '/tmp/app' }])
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
  await assert.rejects(() => cli.run(), (error) => error.code === 'ARKDOWN_CLI_COMMAND_UNSUPPORTED')
})
