'use strict'

const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')

// CompileArkTS/PackageHap declare content inputs and completed outputs. Exercise
// the persisted contract through the public CLI in separate Node processes.
test(
  'compile and archive completion survive processes but reject changed inputs and damaged outputs',
  {
    skip: process.env.ARKDOWN_NATIVE_CACHE_E2E !== '1',
  },
  (context) => {
    const project = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-native-cache-'))
    context.after(() => fs.rmSync(project, { recursive: true, force: true }))
    fs.cpSync(path.resolve(__dirname, '../../../tests/fixtures/minimal-hap'), project, { recursive: true })
    fs.writeFileSync(
      path.join(project, 'oh-package.json5'),
      JSON.stringify({ modelVersion: '6.0.0', dependencies: {}, devDependencies: {} }),
    )
    const source = path.join(project, 'entry/src/main/ets/pages/Index.ets')
    const original = fs.readFileSync(source, 'utf8')
    const abc = path.join(project, 'entry/build/default/intermediates/loader_out/default/ets/modules.abc')
    const archive = path.join(project, 'entry/build/default/outputs/default/entry-default-unsigned.hap')
    const run = (extra = [], success = true, threads = 4) => {
      const result = spawnSync(
        process.execPath,
        [
          path.resolve(__dirname, '../js/arkdown-cli.js'),
          'build',
          '--project',
          project,
          '--target',
          'hap',
          '--mode',
          'debug',
          '--module',
          'entry',
          ...extra,
        ],
        {
          encoding: 'utf8',
          env: { ...process.env, ARKDOWN_LOG: 'info', RAYON_NUM_THREADS: String(threads) },
          timeout: 120000,
          maxBuffer: 8 * 1024 * 1024,
        },
      )
      assert.equal(result.error, undefined, result.error?.message)
      const output = result.stdout + result.stderr
      if (success) assert.equal(result.status, 0, output)
      else assert.notEqual(result.status, 0, output)
      return output
    }
    run([], true, 1)
    const bytecode = fs.readFileSync(abc)
    const marker = new Date(123456000)
    fs.utimesSync(abc, marker, marker)
    fs.utimesSync(archive, marker, marker)
    const hot = run()
    assert.match(hot, /compile cache hit; checker and type facts skipped/)
    assert.match(hot, /package cache hit/)
    assert.equal(fs.statSync(abc).mtimeMs, marker.getTime())
    assert.equal(fs.statSync(archive).mtimeMs, marker.getTime())

    fs.writeFileSync(abc, Buffer.alloc(bytecode.length))
    assert.doesNotMatch(run(), /compile cache hit/)
    assert.deepEqual(fs.readFileSync(abc), bytecode)
    fs.unlinkSync(archive)
    const missingArchive = run()
    assert.match(missingArchive, /compile cache hit/)
    assert.doesNotMatch(missingArchive, /package cache hit/)
    assert.ok(fs.statSync(archive).size > 0)

    const manifest = path.join(
      project,
      'entry/build/default/cache/default/default@CompileArkTS/esmodule/debug/filesInfo.txt',
    )
    const sdkInput = fs.readFileSync(manifest, 'utf8').split('\n')[0].split(';')[0]
    fs.unlinkSync(sdkInput)
    assert.doesNotMatch(run(), /compile cache hit/)
    assert.ok(fs.existsSync(sdkInput), 'a removed SDK input must be regenerated before publishing compile completion')

    fs.writeFileSync(source, original + '\nconst cacheTypeError: number = "wrong";\n')
    const failure = run([], false)
    assert.doesNotMatch(failure, /compile cache hit/)
    assert.match(failure, /not assignable to type 'number'/)
    // A successful facts-only compile cannot serve a later full-check request.
    run(['--skip-checker'])
    const fullAfterSkip = run([], false)
    assert.doesNotMatch(fullAfterSkip, /compile cache hit/)
    assert.match(fullAfterSkip, /not assignable to type 'number'/)
    fs.writeFileSync(source, original)
    assert.doesNotMatch(run(), /compile cache hit/)
    assert.match(run(), /compile cache hit/)
  },
)
