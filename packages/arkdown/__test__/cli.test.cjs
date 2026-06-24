'use strict'

const assert = require('node:assert/strict')
const test = require('node:test')

const { ArkdownPackageCli } = require('../bin/arkdown.js')

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
  ])

  assert.deepEqual(config, {
    project: '/tmp/app',
    compilePluginPath: ['./compile-a.js', './compile-b.js'],
    hvigorPluginFile: './hvigor-plugin.js',
  })
})
