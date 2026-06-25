#!/usr/bin/env node

function createPackageBuild() {
  const nativeBinding = require('../index.js')
  return nativeBinding.build
}

function writeStdout(message) {
  process.stdout.write(`${message}\n`)
}

function writeStderr(message) {
  process.stderr.write(`${message}\n`)
}

class ArkdownPackageCli {
  constructor(argv, build) {
    this.argv = argv
    this.build = build || createPackageBuild()
  }

  async run() {
    const [command, ...args] = this.argv
    if (!command || command === '--help' || command === '-h') {
      this.printHelp()
      return 0
    }
    if (command === '--version' || command === '-V') {
      const { version } = require('../package.json')
      writeStdout(version)
      return 0
    }
    if (command !== 'build') {
      throw new Error(`unsupported command '${command}'. The npm package currently exposes 'build'.`)
    }

    const config = this.parseBuildArgs(args)
    const result = await this.build(config)
    for (const artifact of result.artifacts ?? []) {
      writeStdout(artifact)
    }
    return 0
  }

  parseBuildArgs(args) {
    const config = {}
    for (let i = 0; i < args.length; i += 1) {
      const arg = args[i]
      if (arg === '--help' || arg === '-h') {
        this.printBuildHelp()
        process.exit(0)
      }
      const nextValue = () => {
        const value = args[i + 1]
        if (!value || value.startsWith('-')) {
          throw new Error(`${arg} requires a value`)
        }
        i += 1
        return value
      }
      switch (arg) {
        case '--project':
          config.project = nextValue()
          break
        case '--target':
          config.target = nextValue()
          break
        case '--mode':
          config.mode = nextValue()
          break
        case '--module':
        case '-m':
          config.module = nextValue()
          break
        case '--product':
          config.product = nextValue()
          break
        case '--build-target-name':
          config.buildTargetName = nextValue()
          break
        case '--watch':
          config.watch = true
          break
        case '--preview':
          config.preview = true
          break
        case '--coverage':
          config.coverage = true
          break
        case '--ohos-debug-asan':
          config.ohosDebugAsan = true
          break
        case '--ohos-enable-hwasan':
          config.ohosEnableHwasan = true
          break
        case '--disable-abc-cache':
          config.disableAbcCache = true
          break
        case '--cache-file':
          config.cacheFile = nextValue()
          break
        case '--checker':
          config.checker = nextValue()
          break
        case '--module-analyzer':
          config.moduleAnalyzer = nextValue()
          break
        case '--hvigor-plugin-file':
          appendPathConfig(config, 'hvigorPluginFile', nextValue())
          break
        case '--compile-plugin-path':
          appendPathConfig(config, 'compilePluginPath', nextValue())
          break
        case '--cache':
          config.cache = true
          break
        case '--no-cache':
          config.cache = false
          break
        default:
          throw new Error(`unknown build option '${arg}'`)
      }
    }
    return config
  }

  printHelp() {
    writeStdout(`arkdown

Usage:
  arkdown build [--project <dir>] [--target hap|hsp|har] [--mode debug|release]
  arkdown --version
  arkdown --help`)
  }

  printBuildHelp() {
    writeStdout(`arkdown build

Options:
  --project <dir>     Project root. Defaults to current directory.
  --target <kind>     Output artifact: hap, hsp, or har. Defaults to hap.
  --mode <mode>       Build mode: debug or release. Defaults to debug.
  --module, -m <name> Scope build to one module.
  --product <name>    Product name from build-profile.json5.
  --build-target-name <name>
                      Named build-profile target.
  --watch             Rebuild on changes.
  --preview           Preview resource rebuild mode.
  --coverage          Instrument generated JavaScript for coverage.
  --ohos-debug-asan   Enable AddressSanitizer for the generated package.
  --ohos-enable-hwasan
                      Enable HWAddressSanitizer for the generated package.
  --disable-abc-cache Disable CompileArkTS cache.
  --cache-file <path> Override es2abc cache file.
  --checker <mode>    Checker mode: oxc or off. Defaults to oxc.
  --module-analyzer <mode>
                      Module analyzer: oxc or yuku. Defaults to oxc.
  --hvigor-plugin-file <path>
                      Load a hvigor plugin file before compile plugins.
  --compile-plugin-path <path>
                      Load a hvigor compile plugin path. May be repeated.
  --no-cache          Disable coarse build cache and ABC cache.`)
  }
}

function appendPathConfig(config, key, value) {
  if (config[key] === undefined) {
    config[key] = value
    return
  }
  if (Array.isArray(config[key])) {
    config[key].push(value)
    return
  }
  config[key] = [config[key], value]
}

if (require.main === module) {
  new ArkdownPackageCli(process.argv.slice(2))
    .run()
    .then((code) => {
      process.exitCode = code
    })
    .catch((error) => {
      writeStderr(`error: ${error.message}`)
      process.exitCode = 1
    })
}

module.exports = {
  ArkdownPackageCli,
}
