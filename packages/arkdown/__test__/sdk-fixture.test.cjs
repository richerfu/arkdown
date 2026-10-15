'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')

const { TestSdkFixture } = require('../scripts/prepare-test-sdk.cjs')

test('public SDK checker fixture installs an external callback component without modifying SDK inputs', (context) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-sdk-fixture-'))
  context.after(() => fs.rmSync(root, { recursive: true, force: true }))
  const sdk = path.join(root, 'public-sdk')
  fs.mkdirSync(path.join(sdk, 'ets/build-tools/ets-loader'), { recursive: true })
  const marker = path.join(sdk, 'ets/oh-uni-package.json')
  fs.writeFileSync(marker, '{"apiVersion":20}\n')
  const fixture = new TestSdkFixture().install(sdk)
  assert.equal(fs.readFileSync(marker, 'utf8'), '{"apiVersion":20}\n')
  assert.equal(path.dirname(fixture), fs.realpathSync(root))
  const config = JSON.parse(fs.readFileSync(path.join(fixture, 'ets/sdkConfig.json'), 'utf8'))
  const callbacks = require(path.join(fixture, 'ets', config.apiCheckPlugin[0].path))
  const compatibility = callbacks[config.apiCheckPlugin[0].functionName]
  assert.equal(compatibility('11', '5.0.5(17)').result, true)
  assert.equal(compatibility('6.0.0(20)', '5.0.5(17)').result, false)
  assert.throws(() => compatibility('unknown', '17'), /Unsupported mock SDK version/)
  const checker = new callbacks[config.apiCheckPlugins[0].className]()
  assert.deepEqual(checker.check(), { checkResult: true, checkMessage: '' })
})
