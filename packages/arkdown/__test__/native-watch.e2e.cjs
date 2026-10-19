'use strict'

const assert = require('node:assert/strict')
const { spawn } = require('node:child_process')
const fs = require('node:fs')
const path = require('node:path')
const { setTimeout: delay } = require('node:timers/promises')
const test = require('node:test')

test(
  'snapshot-backed watch modules rebuild after heap reclamation',
  {
    skip: process.env.ARKDOWN_NATIVE_WATCH_E2E !== '1',
    timeout: 120000,
  },
  async (context) => {
    const scratch = path.resolve(__dirname, '../../../target')
    fs.mkdirSync(scratch, { recursive: true })
    const project = fs.mkdtempSync(path.join(scratch, 'arkdown-native-watch-'))
    fs.cpSync(path.resolve(__dirname, '../../../tests/fixtures/minimal-hap'), project, { recursive: true })
    fs.writeFileSync(
      path.join(project, 'oh-package.json5'),
      JSON.stringify({
        modelVersion: '6.0.0',
        dependencies: {},
        devDependencies: {},
      }),
    )
    const source = path.join(project, 'entry/src/main/ets/pages/Index.ets')
    const original = fs.readFileSync(source, 'utf8')
    const abc = path.join(project, 'entry/build/default/intermediates/loader_out/default/ets/modules.abc')
    const child = spawn(
      process.execPath,
      [
        '--expose-gc',
        path.resolve(__dirname, '../js/arkdown-cli.js'),
        'build',
        '--project',
        project,
        '--watch',
        '--no-cache',
      ],
      { env: { ...process.env, ARKDOWN_LOG: 'info,arkdown_napi::heap_reclaimer=debug', ARKDOWN_LOG_CONSOLE: '1' } },
    )
    let log = ''
    child.stdout.on('data', (data) => {
      log += data
    })
    child.stderr.on('data', (data) => {
      log += data
    })
    const closed = new Promise((resolve) => child.on('close', resolve))
    context.after(async () => {
      child.kill('SIGKILL')
      await closed
      fs.rmSync(project, { recursive: true, force: true })
    })
    const waitFor = async (predicate) => {
      const deadline = Date.now() + 45000
      while (!predicate()) {
        assert.equal(child.exitCode, null, log)
        assert.doesNotMatch(log, /watch rebuild failed|ARKDOWN_SOURCE_TEXT_RELEASED/)
        assert.ok(Date.now() < deadline, log)
        await delay(50)
      }
    }
    await waitFor(() => log.includes('session-watcher started'))
    let previous = fs.readFileSync(abc)
    for (const message of ['R11 first rebuild', 'R11 second rebuild']) {
      const logStart = log.length
      fs.writeFileSync(source, original.replace('Hello World', message))
      // Watch keeps the service promise pending; the CLI renders a summary for each completed cycle.
      await waitFor(() => log.slice(logStart).includes('✓ built in '))
      assert.match(log.slice(logStart), /watch rebuild started/)
      const next = fs.readFileSync(abc)
      assert.notDeepEqual(next, previous, 'the edited source must change bytecode')
      previous = next
    }
  },
)
