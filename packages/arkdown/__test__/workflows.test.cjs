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

test('CI builds, tests, and publishes from one workflow', () => {
  const root = path.resolve(__dirname, '../../..')
  const workflowPath = path.join(root, '.github/workflows/publish.yml')
  const workflow = yaml.load(fs.readFileSync(workflowPath, 'utf8'))

  assert.equal(workflow.jobs.package, undefined)
  assert.deepEqual(workflow.jobs.build.needs, ['quality', 'rust'])
  assert.equal(workflow.jobs['test-macos-windows'].needs, 'build')
  assert.equal(workflow.jobs['test-linux'].needs, 'build')
  assert.deepEqual(workflow.jobs.publish.needs, ['test-macos-windows', 'test-linux'])
  assert.equal(workflow.jobs.publish.uses, undefined)
  assert.equal(workflow.jobs.publish.permissions['id-token'], 'write')

  const publishStep = workflow.jobs.publish.steps.find((step) => step.name === 'Publish to npm')
  assert.match(publishStep.run, /napi pre-publish --no-gh-release -t npm\n/)
  assert.doesNotMatch(publishStep.run, /--skip-optional-publish/)

  for (const removed of ['ci.yml', 'publish-beta.yml']) {
    assert.equal(fs.existsSync(path.join(root, '.github/workflows', removed)), false, removed)
  }
})
