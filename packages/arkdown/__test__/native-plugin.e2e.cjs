'use strict'

const assert = require('node:assert/strict')
const childProcess = require('node:child_process')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const test = require('node:test')

const { build } = require('..')

const workspaceRoot = path.resolve(__dirname, '..', '..', '..')
const enabled = process.env.ARKDOWN_NATIVE_PLUGIN_E2E === '1'
const verifyHvigor = process.env.ARKDOWN_HVIGOR_PLUGIN_PARITY === '1'

function prepareCompilePluginProject(project) {
  fs.cpSync(path.join(workspaceRoot, 'tests', 'fixtures', 'minimal-hap'), project, { recursive: true })
  const compileRoot = path.join(project, 'entry', 'compile-plugins')
  fs.mkdirSync(compileRoot, { recursive: true })
  fs.copyFileSync(
    path.join(workspaceRoot, 'tests', 'fixtures', 'compile-plugins', 'lifecycle.ts'),
    path.join(compileRoot, 'lifecycle.ts'),
  )
  fs.writeFileSync(
    path.join(project, 'entry', 'hvigorfile.ts'),
    `
import { getNode } from '@ohos/hvigor'
import { hapTasks, OhosPluginId } from '@ohos/hvigor-ohos-plugin'

const loadCompilePlugin = {
  pluginId: 'fixture.load-compile-plugin',
  apply(node: any) {
    node.getContext(OhosPluginId.OHOS_HAP_PLUGIN)
      .loadCompilePlugin('./compile-plugins/lifecycle.ts')
  },
}

export default { system: hapTasks, plugins: [loadCompilePlugin] }
`,
  )
}

function prepareTaskLifecycleProject(project) {
  fs.cpSync(path.join(workspaceRoot, 'tests', 'fixtures', 'minimal-hap'), project, { recursive: true })
  fs.rmSync(path.join(project, 'entry', 'build'), { recursive: true, force: true })
  fs.writeFileSync(
    path.join(project, 'entry', 'hvigorfile.ts'),
    `
import { getNode, hvigor } from '@ohos/hvigor'
import { hapTasks } from '@ohos/hvigor-ohos-plugin'
import * as fs from 'node:fs'
import * as path from 'node:path'

const node = getNode(__filename)!
const tracePath = process.env.ARKDOWN_TASK_TRACE!
const packagePath = path.join(node.getNodePath(), 'build', 'default', 'outputs', 'default', 'entry-default-unsigned.hap')
const record = (event: string) => fs.appendFileSync(tracePath, event + '\\n')

hvigor.nodesEvaluated(() => {
  node.registerTask({
    name: 'default@fixtureBeforePackage',
    beforeRun() { record('custom-before') },
    run() { record('custom-run') },
    afterRun() { record('custom-after') },
    postDependencies: ['default@PackageHap'],
  })
  const packageTask = node.getTaskByName('default@PackageHap')!
  packageTask.beforeRun(() => record('package-before:' + fs.existsSync(packagePath)))
  packageTask.afterRun(() => record('package-after:' + fs.existsSync(packagePath)))
})

export default { system: hapTasks, plugins: [] }
`,
  )
}

function prepareIncrementalTaskProject(project) {
  fs.cpSync(path.join(workspaceRoot, 'tests', 'fixtures', 'minimal-hap'), project, { recursive: true })
  fs.rmSync(path.join(project, '.hvigor'), { recursive: true, force: true })
  fs.rmSync(path.join(project, 'entry', 'build'), { recursive: true, force: true })
  fs.writeFileSync(path.join(project, 'fixture-input.txt'), 'first')
  fs.writeFileSync(
    path.join(project, 'entry', 'hvigorfile.ts'),
    `
import { getNode, hvigor } from '@ohos/hvigor'
import { hapTasks } from '@ohos/hvigor-ohos-plugin'
import * as fs from 'node:fs'
import * as path from 'node:path'

const node = getNode(__filename)!
const inputPath = path.join(node.getParentNode().getNodePath(), 'fixture-input.txt')
const outputPath = path.join(node.getParentNode().getNodePath(), 'fixture-output.txt')
const record = (event: string) => fs.appendFileSync(process.env.ARKDOWN_INCREMENTAL_TASK_TRACE!, event + '\\n')

hvigor.nodesEvaluated(() => {
  node.registerTask({
    name: 'default@fixtureIncremental',
    input(input: any) { input.file(inputPath).property('flavor', 'default') },
    output(output: any) { output.file(outputPath) },
    beforeRun() { record('before') },
    run() {
      fs.copyFileSync(inputPath, outputPath)
      record('run')
    },
    afterRun() { record('after') },
    postDependencies: ['default@PackageHap'],
  })
})

export default { system: hapTasks, plugins: [] }
`,
  )
}

function traceLineCount(file) {
  if (!fs.existsSync(file)) return 0
  const content = fs.readFileSync(file, 'utf8').trim()
  return content.length === 0 ? 0 : content.split('\n').length
}

async function waitForWatchCondition(child, condition, description, diagnostics, timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    if (condition()) return
    if (child.exitCode !== null) {
      throw new Error(`watch process exited before ${description} (code ${child.exitCode})\n${diagnostics()}`)
    }
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw new Error(`timed out waiting for ${description}\n${diagnostics()}`)
}

function prepareTransformAbcProject(project) {
  fs.cpSync(path.join(workspaceRoot, 'tests', 'fixtures', 'minimal-hap'), project, { recursive: true })
  fs.rmSync(path.join(project, 'entry', 'build'), { recursive: true, force: true })
  fs.writeFileSync(
    path.join(project, 'entry', 'hvigorfile.ts'),
    `
import { hapTasks, OhosPluginId } from '@ohos/hvigor-ohos-plugin'
import * as fs from 'node:fs'
import * as path from 'node:path'

const plugin = {
  pluginId: 'fixture.transform-abc',
  apply(node: any) {
    const context = node.getContext(OhosPluginId.OHOS_HAP_PLUGIN)
    context.transformAbc(async (abcPath: string, config: any) => {
      await Promise.resolve()
      fs.writeFileSync(process.env.ARKDOWN_TRANSFORM_ABC_TRACE!, JSON.stringify({
        fileName: path.basename(abcPath),
        exists: fs.existsSync(abcPath),
        isArkGuardEnabled: config.isArkGuardEnabled,
      }))
    }, 'default')
  },
}

export default { system: hapTasks, plugins: [plugin] }
`,
  )
}

function prepareContextProject(project) {
  fs.cpSync(path.join(workspaceRoot, 'tests', 'fixtures', 'minimal-hap'), project, { recursive: true })
  fs.rmSync(path.join(project, 'build'), { recursive: true, force: true })
  fs.rmSync(path.join(project, 'entry', 'build'), { recursive: true, force: true })
  fs.writeFileSync(
    path.join(project, 'hvigorfile.ts'),
    `
import { appTasks, OhosPluginId } from '@ohos/hvigor-ohos-plugin'
import * as fs from 'node:fs'

const plugin = {
  pluginId: 'fixture.project-context',
  apply(node: any) {
    const context = node.getContext(OhosPluginId.OHOS_APP_PLUGIN)
    const profile = context.getBuildProfileOpt()
    profile.app.products[0].bundleName = 'com.example.context'
    context.setBuildProfileOpt(profile)
    const app = context.getAppJsonOpt()
    app.app.versionCode = 2000002
    app.app.versionName = '2.0.2'
    context.setAppJsonOpt(app)
    context.setDependenciesOpt(context.getDependenciesOpt() || {})
    context.setDevDependenciesOpt(context.getDevDependenciesOpt() || {})
    context.setDynamicDependenciesOpt(context.getDynamicDependenciesOpt() || {})
    context.setOverrides(context.getOverrides() || {})
    fs.appendFileSync(process.env.ARKDOWN_CONTEXT_TRACE!, JSON.stringify({
      owner: 'project',
      hash: context.getAllModuleNameHash(),
      dependencies: context.getOhpmDependencyInfo(),
      remote: context.getOhpmRemoteHspDependencyInfo(false),
      signed: context.getOhpmRemoteHspDependencyInfo(true),
      versionCode: context.getAppJsonOpt().app.versionCode,
      bundleName: context.getBuildProfileOpt().app.products[0].bundleName,
    }) + '\\n')
  },
}

export default { system: appTasks, plugins: [plugin] }
`,
  )
  fs.writeFileSync(
    path.join(project, 'entry', 'hvigorfile.ts'),
    `
import { hapTasks, OhosPluginId } from '@ohos/hvigor-ohos-plugin'
import * as fs from 'node:fs'

const plugin = {
  pluginId: 'fixture.module-context',
  apply(node: any) {
    const context = node.getContext(OhosPluginId.OHOS_HAP_PLUGIN)
    const profile = context.getBuildProfileOpt()
    profile.buildOption = {
      ...(profile.buildOption || {}),
      resOptions: {
        ...(profile.buildOption?.resOptions || {}),
        copyCodeResource: { enable: true },
      },
    }
    context.setBuildProfileOpt(profile)
    const moduleJson = context.getModuleJsonOpt()
    moduleJson.module.description = '$string:EntryAbility_desc'
    context.setModuleJsonOpt(moduleJson)
    context.setDependenciesOpt(context.getDependenciesOpt() || {})
    context.setDevDependenciesOpt(context.getDevDependenciesOpt() || {})
    context.setDynamicDependenciesOpt(context.getDynamicDependenciesOpt() || {})
    context.setVersion('2.0.0')
    const projectContext = node.getParentNode().getContext(OhosPluginId.OHOS_APP_PLUGIN)
    fs.appendFileSync(process.env.ARKDOWN_CONTEXT_TRACE!, JSON.stringify({
      owner: 'module',
      hashAfterMutation: projectContext.getAllModuleNameHash(),
      dependencies: context.getOhpmDependencyInfo(),
      remote: context.getOhpmRemoteHspDependencyInfo(false),
      signed: context.getOhpmRemoteHspDependencyInfo(true),
      version: context.getVersion(),
      description: context.getModuleJsonOpt().module.description,
      copyCodeResource: context.getBuildProfileOpt().buildOption.resOptions.copyCodeResource.enable,
    }) + '\\n')
  },
}

export default { system: hapTasks, plugins: [plugin] }
`,
  )
}

test(
  'OhosModuleContext TypeScript compile plugin executes through the Rolldown driver',
  { skip: !enabled },
  async (context) => {
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-native-plugin-'))
    const project = path.join(tempRoot, 'minimal-hap')
    const tracePath = path.join(tempRoot, 'plugin-trace.json')
    context.after(() => fs.rmSync(tempRoot, { recursive: true, force: true }))
    prepareCompilePluginProject(project)

    const priorTrace = process.env.ARKDOWN_PLUGIN_TRACE
    const priorInvalidate = process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE
    process.env.ARKDOWN_PLUGIN_TRACE = tracePath
    delete process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE
    context.after(() => {
      if (priorTrace === undefined) delete process.env.ARKDOWN_PLUGIN_TRACE
      else process.env.ARKDOWN_PLUGIN_TRACE = priorTrace
      if (priorInvalidate === undefined) delete process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE
      else process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE = priorInvalidate
    })

    const config = {
      project,
      target: 'hap',
      mode: 'debug',
      module: 'entry',
      product: 'default',
      buildTargetName: 'default',
      cache: true,
    }
    let hvigorTrace
    if (verifyHvigor) {
      const hvigorProject = path.join(tempRoot, 'hvigor-minimal-hap')
      const hvigorTracePath = path.join(tempRoot, 'hvigor-plugin-trace.json')
      prepareCompilePluginProject(hvigorProject)
      const hvigorw = process.env.ARKDOWN_PARITY_HVIGORW
      assert.ok(hvigorw, 'ARKDOWN_PARITY_HVIGORW is required when ARKDOWN_HVIGOR_PLUGIN_PARITY=1')
      const command = childProcess.spawnSync(
        hvigorw,
        ['--mode', 'module', '-p', 'product=default', '-p', 'buildMode=debug', 'assembleHap', '--no-daemon'],
        {
          cwd: hvigorProject,
          encoding: 'utf8',
          env: { ...process.env, ARKDOWN_PLUGIN_TRACE: hvigorTracePath },
        },
      )
      assert.equal(command.status, 0, `latest hvigor failed:\n${command.stdout}\n${command.stderr}`)
      hvigorTrace = JSON.parse(fs.readFileSync(hvigorTracePath, 'utf8'))
    }
    const first = await build(config)
    assert.equal(
      first.artifacts.some((artifact) => artifact.endsWith('.hap')),
      true,
    )

    const firstTrace = JSON.parse(fs.readFileSync(tracePath, 'utf8'))
    assert.deepEqual(firstTrace.events, ['buildStart', 'beforeBuildEnd', 'buildEnd'])
    assert.equal(firstTrace.buildStartArgumentCount, 0)
    assert.equal(firstTrace.beforeBuildEndArgumentCount, 1)
    assert.equal(firstTrace.buildEndArgumentCount, 1)
    assert.equal(firstTrace.buildEndError, undefined)
    assert.equal(firstTrace.resolveIdCount > 0, true)
    assert.equal(firstTrace.loadCount > 0, true)
    assert.equal(firstTrace.transformCount > 0, true)
    assert.equal(firstTrace.moduleParsedCount > 0, true)
    assert.equal(firstTrace.moduleIds.length > 0, true)
    assert.equal(firstTrace.moduleInfoCount, firstTrace.moduleIds.length)
    assert.equal(firstTrace.parsedType, 'Program')
    assert.equal(firstTrace.projectConfig.moduleName, 'entry')
    if (hvigorTrace) {
      assert.deepEqual(firstTrace.events, hvigorTrace.events)
      assert.equal(firstTrace.buildStartArgumentCount, hvigorTrace.buildStartArgumentCount)
      assert.equal(firstTrace.beforeBuildEndArgumentCount, hvigorTrace.beforeBuildEndArgumentCount)
      assert.equal(firstTrace.buildEndArgumentCount, hvigorTrace.buildEndArgumentCount)
      assert.equal(firstTrace.parsedType, hvigorTrace.parsedType)
      assert.equal(hvigorTrace.resolveIdCount > 0, true)
      assert.equal(hvigorTrace.loadCount > 0, true)
      assert.equal(hvigorTrace.transformCount > 0, true)
      assert.equal(hvigorTrace.moduleParsedCount > 0, true)
    }

    process.env.ARKDOWN_PLUGIN_FORCE_INVALIDATE = '1'
    await build(config)
    const secondTrace = JSON.parse(fs.readFileSync(tracePath, 'utf8'))
    assert.equal(secondTrace.shouldInvalidCacheCount > 0, true)
    assert.equal(typeof secondTrace.cacheRequest.id, 'string')
    assert.equal(secondTrace.cacheRequest.id.length > 0, true)
    assert.equal(secondTrace.cacheRequest.code.length > 0, true)
    assert.equal(secondTrace.cacheRequest.astType, 'Program')
  },
)

test('TypeScript custom tasks execute at the native PackageHap boundary', { skip: !enabled }, async (context) => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-native-task-'))
  const project = path.join(tempRoot, 'minimal-hap')
  const tracePath = path.join(tempRoot, 'arkdown-task-trace.txt')
  context.after(() => fs.rmSync(tempRoot, { recursive: true, force: true }))
  prepareTaskLifecycleProject(project)

  const priorTrace = process.env.ARKDOWN_TASK_TRACE
  process.env.ARKDOWN_TASK_TRACE = tracePath
  context.after(() => {
    if (priorTrace === undefined) delete process.env.ARKDOWN_TASK_TRACE
    else process.env.ARKDOWN_TASK_TRACE = priorTrace
  })

  const result = await build({
    project,
    target: 'hap',
    mode: 'debug',
    module: 'entry',
    product: 'default',
    buildTargetName: 'default',
    cache: false,
  })
  assert.equal(
    result.artifacts.some((artifact) => artifact.endsWith('.hap')),
    true,
  )
  const arkdownTrace = fs.readFileSync(tracePath, 'utf8').trim().split('\n')
  assert.deepEqual(arkdownTrace, [
    'custom-before',
    'custom-run',
    'custom-after',
    'package-before:false',
    'package-after:true',
  ])

  if (verifyHvigor) {
    const hvigorProject = path.join(tempRoot, 'hvigor-minimal-hap')
    const hvigorTracePath = path.join(tempRoot, 'hvigor-task-trace.txt')
    prepareTaskLifecycleProject(hvigorProject)
    const hvigorw = process.env.ARKDOWN_PARITY_HVIGORW
    assert.ok(hvigorw, 'ARKDOWN_PARITY_HVIGORW is required when ARKDOWN_HVIGOR_PLUGIN_PARITY=1')
    const command = childProcess.spawnSync(
      hvigorw,
      ['--mode', 'module', '-p', 'product=default', '-p', 'buildMode=debug', 'assembleHap', '--no-daemon'],
      {
        cwd: hvigorProject,
        encoding: 'utf8',
        env: { ...process.env, ARKDOWN_TASK_TRACE: hvigorTracePath },
      },
    )
    assert.equal(command.status, 0, `latest hvigor failed:\n${command.stdout}\n${command.stderr}`)
    assert.deepEqual(fs.readFileSync(hvigorTracePath, 'utf8').trim().split('\n'), arkdownTrace)
  }
})

test(
  'TypeScript custom task snapshots match latest hvigor incremental execution',
  { skip: !enabled },
  async (context) => {
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-native-incremental-task-'))
    const project = path.join(tempRoot, 'arkdown', 'minimal-hap')
    const tracePath = path.join(tempRoot, 'arkdown-incremental-task-trace.txt')
    context.after(() => fs.rmSync(tempRoot, { recursive: true, force: true }))
    prepareIncrementalTaskProject(project)

    const priorTrace = process.env.ARKDOWN_INCREMENTAL_TASK_TRACE
    process.env.ARKDOWN_INCREMENTAL_TASK_TRACE = tracePath
    context.after(() => {
      if (priorTrace === undefined) delete process.env.ARKDOWN_INCREMENTAL_TASK_TRACE
      else process.env.ARKDOWN_INCREMENTAL_TASK_TRACE = priorTrace
    })

    const config = {
      project,
      target: 'hap',
      mode: 'debug',
      module: 'entry',
      product: 'default',
      buildTargetName: 'default',
      cache: true,
    }
    const arkdownCounts = []
    await build(config)
    arkdownCounts.push(fs.readFileSync(tracePath, 'utf8').trim().split('\n').length)
    await build(config)
    arkdownCounts.push(fs.readFileSync(tracePath, 'utf8').trim().split('\n').length)
    fs.writeFileSync(path.join(project, 'fixture-input.txt'), 'second-value')
    await build(config)
    arkdownCounts.push(fs.readFileSync(tracePath, 'utf8').trim().split('\n').length)
    fs.rmSync(path.join(project, 'fixture-output.txt'))
    await build(config)
    arkdownCounts.push(fs.readFileSync(tracePath, 'utf8').trim().split('\n').length)
    assert.deepEqual(arkdownCounts, [3, 3, 6, 9])

    if (verifyHvigor) {
      const hvigorProject = path.join(tempRoot, 'hvigor', 'minimal-hap')
      const hvigorTracePath = path.join(tempRoot, 'hvigor-incremental-task-trace.txt')
      prepareIncrementalTaskProject(hvigorProject)
      const hvigorw = process.env.ARKDOWN_PARITY_HVIGORW
      assert.ok(hvigorw, 'ARKDOWN_PARITY_HVIGORW is required when ARKDOWN_HVIGOR_PLUGIN_PARITY=1')
      const runHvigor = () =>
        childProcess.spawnSync(
          hvigorw,
          ['--mode', 'module', '-p', 'product=default', '-p', 'buildMode=debug', 'assembleHap', '--no-daemon'],
          {
            cwd: hvigorProject,
            encoding: 'utf8',
            env: { ...process.env, ARKDOWN_INCREMENTAL_TASK_TRACE: hvigorTracePath },
          },
        )
      const hvigorCounts = []
      for (let iteration = 0; iteration < 2; iteration++) {
        const command = runHvigor()
        assert.equal(command.status, 0, `latest hvigor failed:\n${command.stdout}\n${command.stderr}`)
        hvigorCounts.push(fs.readFileSync(hvigorTracePath, 'utf8').trim().split('\n').length)
      }
      fs.writeFileSync(path.join(hvigorProject, 'fixture-input.txt'), 'second-value')
      let command = runHvigor()
      assert.equal(command.status, 0, `latest hvigor failed:\n${command.stdout}\n${command.stderr}`)
      hvigorCounts.push(fs.readFileSync(hvigorTracePath, 'utf8').trim().split('\n').length)
      fs.rmSync(path.join(hvigorProject, 'fixture-output.txt'))
      command = runHvigor()
      assert.equal(command.status, 0, `latest hvigor failed:\n${command.stdout}\n${command.stderr}`)
      hvigorCounts.push(fs.readFileSync(hvigorTracePath, 'utf8').trim().split('\n').length)
      assert.deepEqual(hvigorCounts, arkdownCounts)
    }
  },
)

test('watch rebuilds when an arbitrary custom TaskInput file changes', { skip: !enabled }, async (context) => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-native-task-watch-'))
  const project = path.join(tempRoot, 'minimal-hap')
  const tracePath = path.join(tempRoot, 'arkdown-watch-task-trace.txt')
  context.after(() => fs.rmSync(tempRoot, { recursive: true, force: true }))
  prepareIncrementalTaskProject(project)

  const packageRoot = path.resolve(__dirname, '..')
  const config = {
    project,
    target: 'hap',
    mode: 'debug',
    module: 'entry',
    product: 'default',
    buildTargetName: 'default',
    cache: true,
    watch: true,
  }
  const script = `
const { build } = require(${JSON.stringify(packageRoot)})
build(${JSON.stringify(config)}).then(
  () => { process.stderr.write('watch build returned unexpectedly\\n'); process.exitCode = 2 },
  (error) => { process.stderr.write(String(error && error.stack || error) + '\\n'); process.exitCode = 1 },
)
`
  const child = childProcess.spawn(process.execPath, ['-e', script], {
    cwd: project,
    env: {
      ...process.env,
      ARKDOWN_INCREMENTAL_TASK_TRACE: tracePath,
      ARKDOWN_LOG: 'info',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  let stdout = ''
  let stderr = ''
  child.stdout.setEncoding('utf8')
  child.stderr.setEncoding('utf8')
  child.stdout.on('data', (chunk) => {
    stdout += chunk
  })
  child.stderr.on('data', (chunk) => {
    stderr += chunk
  })
  const diagnostics = () => `stdout:\n${stdout}\nstderr:\n${stderr}`
  const exited = new Promise((resolve) => child.once('exit', resolve))

  try {
    await waitForWatchCondition(
      child,
      () => traceLineCount(tracePath) === 3 && stderr.includes('session-watcher started'),
      'the initial build and watcher startup',
      diagnostics,
    )
    fs.writeFileSync(path.join(project, 'fixture-input.txt'), 'changed-under-watch')
    await waitForWatchCondition(
      child,
      () => traceLineCount(tracePath) === 6,
      'the custom task watch rebuild',
      diagnostics,
    )
    assert.equal(fs.readFileSync(path.join(project, 'fixture-output.txt'), 'utf8'), 'changed-under-watch')
    assert.equal(child.exitCode, null, diagnostics())
  } finally {
    if (child.exitCode === null) child.kill()
    await Promise.race([exited, new Promise((resolve) => setTimeout(resolve, 5_000))])
  }
})

test('OhosModuleContext.transformAbc executes after ABC generation', { skip: !enabled }, async (context) => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-transform-abc-'))
  const project = path.join(tempRoot, 'minimal-hap')
  const tracePath = path.join(tempRoot, 'arkdown-transform-abc.json')
  context.after(() => fs.rmSync(tempRoot, { recursive: true, force: true }))
  prepareTransformAbcProject(project)

  const priorTrace = process.env.ARKDOWN_TRANSFORM_ABC_TRACE
  process.env.ARKDOWN_TRANSFORM_ABC_TRACE = tracePath
  context.after(() => {
    if (priorTrace === undefined) delete process.env.ARKDOWN_TRANSFORM_ABC_TRACE
    else process.env.ARKDOWN_TRANSFORM_ABC_TRACE = priorTrace
  })

  const result = await build({
    project,
    target: 'hap',
    mode: 'debug',
    module: 'entry',
    product: 'default',
    buildTargetName: 'default',
    cache: false,
  })
  assert.equal(
    result.artifacts.some((artifact) => artifact.endsWith('.hap')),
    true,
  )
  const arkdownTrace = JSON.parse(fs.readFileSync(tracePath, 'utf8'))
  assert.deepEqual(arkdownTrace, {
    fileName: 'modules.abc',
    exists: true,
    isArkGuardEnabled: false,
  })

  if (verifyHvigor) {
    const hvigorProject = path.join(tempRoot, 'hvigor-minimal-hap')
    const hvigorTracePath = path.join(tempRoot, 'hvigor-transform-abc.json')
    prepareTransformAbcProject(hvigorProject)
    const hvigorw = process.env.ARKDOWN_PARITY_HVIGORW
    assert.ok(hvigorw, 'ARKDOWN_PARITY_HVIGORW is required when ARKDOWN_HVIGOR_PLUGIN_PARITY=1')
    const command = childProcess.spawnSync(
      hvigorw,
      ['--mode', 'module', '-p', 'product=default', '-p', 'buildMode=debug', 'assembleHap', '--no-daemon'],
      {
        cwd: hvigorProject,
        encoding: 'utf8',
        env: { ...process.env, ARKDOWN_TRANSFORM_ABC_TRACE: hvigorTracePath },
      },
    )
    assert.equal(command.status, 0, `latest hvigor failed:\n${command.stdout}\n${command.stderr}`)
    assert.deepEqual(JSON.parse(fs.readFileSync(hvigorTracePath, 'utf8')), arkdownTrace)
  }
})

test(
  'OHOS project and module contexts match latest hvigor loader mutations and graph getters',
  { skip: !enabled },
  async (context) => {
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'arkdown-context-'))
    const project = path.join(tempRoot, 'arkdown', 'minimal-hap')
    const tracePath = path.join(tempRoot, 'arkdown-context.jsonl')
    context.after(() => fs.rmSync(tempRoot, { recursive: true, force: true }))
    prepareContextProject(project)

    const priorTrace = process.env.ARKDOWN_CONTEXT_TRACE
    process.env.ARKDOWN_CONTEXT_TRACE = tracePath
    context.after(() => {
      if (priorTrace === undefined) delete process.env.ARKDOWN_CONTEXT_TRACE
      else process.env.ARKDOWN_CONTEXT_TRACE = priorTrace
    })

    const result = await build({
      project,
      target: 'hap',
      mode: 'debug',
      module: 'entry',
      product: 'default',
      buildTargetName: 'default',
      cache: false,
    })
    assert.equal(
      result.artifacts.some((artifact) => artifact.endsWith('.hap')),
      true,
    )
    const arkdownTrace = fs
      .readFileSync(tracePath, 'utf8')
      .trim()
      .split('\n')
      .map((line) => JSON.parse(line))
    assert.equal(arkdownTrace.length, 2)
    assert.equal(arkdownTrace[0].hash, 'd729bd21d61b1a63a454a43c7ec0b7a7a3548edbe83d35833d8e04ba937bcd0f')
    assert.equal(arkdownTrace[1].hashAfterMutation, arkdownTrace[0].hash)
    assert.equal(arkdownTrace[0].versionCode, 2000002)
    assert.equal(arkdownTrace[1].version, '2.0.0')

    if (verifyHvigor) {
      const hvigorProject = path.join(tempRoot, 'hvigor', 'minimal-hap')
      const hvigorTracePath = path.join(tempRoot, 'hvigor-context.jsonl')
      prepareContextProject(hvigorProject)
      const hvigorw = process.env.ARKDOWN_PARITY_HVIGORW
      assert.ok(hvigorw, 'ARKDOWN_PARITY_HVIGORW is required when ARKDOWN_HVIGOR_PLUGIN_PARITY=1')
      const command = childProcess.spawnSync(
        hvigorw,
        ['--mode', 'module', '-p', 'product=default', '-p', 'buildMode=debug', 'assembleHap', '--no-daemon'],
        {
          cwd: hvigorProject,
          encoding: 'utf8',
          env: { ...process.env, ARKDOWN_CONTEXT_TRACE: hvigorTracePath },
        },
      )
      assert.equal(command.status, 0, `latest hvigor failed:\n${command.stdout}\n${command.stderr}`)
      const hvigorTrace = fs
        .readFileSync(hvigorTracePath, 'utf8')
        .trim()
        .split('\n')
        .map((line) => JSON.parse(line))
      assert.deepEqual(arkdownTrace, hvigorTrace)
    }
  },
)
