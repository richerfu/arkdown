'use strict'

const assert = require('node:assert/strict')
const test = require('node:test')
const { BuildLogDispatcher, TerminalColors } = require('../js/build-logger.js')

function capture(stdoutTty, stderrTty, env = {}) {
  const text = { stdout: '', stderr: '' }
  return {
    text,
    output: {
      stdout: {
        isTTY: stdoutTty,
        write(line) {
          text.stdout += line
        },
      },
      stderr: {
        isTTY: stderrTty,
        write(line) {
          text.stderr += line
        },
      },
      env,
    },
  }
}

test('default API log rendering uses the output stream color capability', () => {
  for (const [stdoutTty, stderrTty] of [
    [true, false],
    [false, true],
  ]) {
    const { text, output } = capture(stdoutTty, stderrTty)
    const logger = new BuildLogDispatcher({ logLevel: 'debug' }, output)
    for (const level of ['info', 'debug', 'warn', 'error']) {
      logger.emit({ level, target: 'arkdown::plugin', message: level, fields: { plugin: 'fixture' } })
    }
    assert.equal(text.stdout.includes('\x1b[36m'), stdoutTty)
    assert.equal(text.stdout.includes('\x1b[90m'), stdoutTty)
    assert.equal(text.stderr.includes('\x1b[33m'), stderrTty)
    assert.equal(text.stderr.includes('\x1b[31m'), stderrTty)
    assert.ok(text.stdout.includes('info plugin=fixture'))
    assert.ok(text.stderr.includes('warn plugin=fixture'))
  }
})

test('custom log callbacks receive raw data and silent mode emits no color sequences', () => {
  const { text, output } = capture(true, true, { FORCE_COLOR: '1' })
  const events = []
  const log = { level: 'warn', target: 'arkdown::plugin', message: 'warning', fields: { plugin: 'fixture' } }
  new BuildLogDispatcher({ onLog: (event) => events.push(event) }, output).emit(log)
  assert.deepEqual(events, [log])
  assert.equal(events[0].message.includes('\x1b'), false)
  new BuildLogDispatcher({ logLevel: 'silent' }, output).emit(log)
  assert.deepEqual(text, { stdout: '', stderr: '' })
})

test('nested and multiline log colors restore the outer style and reset at line boundaries', () => {
  const colors = new TerminalColors({ isTTY: true }, {})
  const result = colors.format('error', `first\n${colors.format('warn', 'nested')} tail`)
  assert.equal(result, '\x1b[31mfirst\x1b[39m\n\x1b[31m\x1b[33mnested\x1b[31m tail\x1b[39m')
})
