'use strict'

const nativeBinding = require('../index.js')
const { createBuild, createHvigorCommand, HVIGOR_PARITY_VERSION } = require('./runtime.js')
const nativeBuild = nativeBinding.build

module.exports = nativeBinding
const nativeRuntime = {
  runRuntimeTasks: nativeBinding.runRuntimeTasks,
}
module.exports.build = createBuild(nativeBuild)
module.exports.runHvigorCommand = createHvigorCommand(nativeBuild, nativeRuntime)
module.exports.hvigorVersion = HVIGOR_PARITY_VERSION
