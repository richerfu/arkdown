'use strict'

const nativeBinding = require('../index.js')
const { createBuild, createCheck } = require('./plugin-adapter.js')

module.exports = nativeBinding
module.exports.build = createBuild(nativeBinding.build, nativeBinding)
module.exports.check = createCheck(nativeBinding.check)
