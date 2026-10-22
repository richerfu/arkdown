'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const test = require('node:test')
const yaml = require('js-yaml')

function matrixValues(matrix, keys) {
  return keys.reduce((values, key) => values.flatMap((value) => value?.[key] ?? []), [matrix])
}

test('every workflow job uses a GitHub-hosted runner or a checked local workflow', () => {
  const root = path.resolve(__dirname, '../../..')
  const directory = path.join(root, '.github/workflows')
  const workflows = fs
    .readdirSync(directory)
    .filter((name) => /\.ya?ml$/.test(name))
    .sort()
  const hostedRunners = new Set(['ubuntu-latest', 'macos-latest', 'windows-latest'])

  assert.ok(workflows.length > 0)
  for (const name of workflows) {
    const workflow = yaml.load(fs.readFileSync(path.join(directory, name), 'utf8'))
    for (const [id, job] of Object.entries(workflow.jobs)) {
      const location = `${name}: ${id}`
      if (job.uses) {
        assert.match(job.uses, /^\.\/\.github\/workflows\/[^/]+\.ya?ml$/, location)
        assert.ok(workflows.includes(path.basename(job.uses)), location)
        continue
      }

      const runner = job['runs-on']
      assert.equal(typeof runner, 'string', `${location}: runner groups and label arrays are forbidden`)
      const expression = runner.match(/^\$\{\{\s*matrix\.([\w.]+)\s*\}\}$/)
      let runners = [runner]
      if (expression) {
        const keys = expression[1].split('.')
        const matrix = job.strategy?.matrix
        assert.ok(matrix && typeof matrix === 'object', `${location}: runner matrix must be declared locally`)
        runners = matrixValues(matrix, keys)
        for (const row of matrix.include ?? []) {
          runners.push(...matrixValues(row, keys))
        }
      }
      assert.ok(runners.length > 0, `${location}: runner matrix must not be empty`)
      for (const label of runners) {
        assert.ok(hostedRunners.has(label), `${location}: unsupported runner ${JSON.stringify(label)}`)
      }
    }
  }
})

test('CI packaging waits for lint and the Rust platform matrix', () => {
  const root = path.resolve(__dirname, '../../..')
  const workflow = yaml.load(fs.readFileSync(path.join(root, '.github/workflows/ci.yml'), 'utf8'))
  assert.deepEqual(workflow.jobs.package.needs, ['quality', 'rust'])
  assert.equal(workflow.jobs.package.uses, './.github/workflows/publish.yml')
  assert.equal(workflow.jobs.package.if, undefined)
})
