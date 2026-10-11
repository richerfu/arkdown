'use strict'

const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')

test(
  'embedded checker reports SDK diagnostics and releases its Node callback host',
  {
    skip: process.env.ARKDOWN_NATIVE_CHECKER_E2E !== '1',
  },
  (context) => {
    const project = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-crate-check-'))
    context.after(() => fs.rmSync(project, { recursive: true, force: true }))
    fs.cpSync(path.resolve(__dirname, '../../../tests/fixtures/minimal-hap'), project, { recursive: true })
    fs.writeFileSync(
      path.join(project, 'oh-package.json5'),
      JSON.stringify({
        modelVersion: '6.0.0',
        dependencies: {},
        devDependencies: {},
      }),
    )
    // A leaked ThreadsafeFunction leaves the child alive after the assertions pass.
    // Require natural process exit: a forced test-runner exit would hide that bug.
    const result = spawnSync(
      process.execPath,
      [
        '-e',
        `
    const assert = require('node:assert/strict')
    const fs = require('node:fs')
    const path = require('node:path')
    const { check } = require('./js/runtime-entry')
    const project = process.argv[1]
    const source = path.join(project, 'entry/src/main/ets/pages/Index.ets')
    const original = fs.readFileSync(source, 'utf8')
    ;(async () => {
      await check({ project })
      fs.writeFileSync(source, original + '\\nconst embeddedTypeError: number = "wrong";\\n')
      await assert.rejects(check({ project }), /not assignable to type 'number'/)
      fs.writeFileSync(source, original)
      await check({ project })
    })().catch((error) => { console.error(error); process.exitCode = 1 })
  `,
        project,
      ],
      { cwd: path.resolve(__dirname, '..'), encoding: 'utf8', timeout: 60000 },
    )
    assert.equal(result.error, undefined, result.error?.message)
    assert.equal(result.status, 0, result.stdout + result.stderr)
  },
)
