'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const test = require('node:test')

test('JavaScript remains plugin glue around Rust OXC Rolldown planners', () => {
  const runtime = fs.readFileSync(path.resolve(__dirname, '../runtime.js'), 'utf8')
  const cli = fs.readFileSync(path.resolve(__dirname, '../bin/arkdown.js'), 'utf8')

  for (const removed of [
    'typescript.transpileModule',
    "require('acorn')",
    'function collectCompilePluginPaths',
    'function resolvePluginPath',
    'function validateHvigorCompilePlugin',
  ]) {
    assert.equal(runtime.includes(removed), false, `${removed} must not return to the JavaScript runtime`)
  }

  for (const nativeBoundary of [
    'compileHvigorScriptSync',
    'parseHvigorJsonText',
    'planHvigorCompilePlugins',
    'collectBuildScriptPluginPaths',
    'prepareNativeBuildConfig',
    'mergeHvigorConfig',
  ]) {
    assert.equal(runtime.includes(`nativeRuntimeBinding.${nativeBoundary}`), true)
  }

  assert.equal(cli.includes('parseHvigorCliArgs(args)'), true)
  assert.equal(cli.includes('parseArkdownBuildCliArgs(args)'), true)
  assert.equal(cli.includes('const valueOptions = new Map'), false)
  assert.equal(cli.includes('switch (arg)'), false)
})
