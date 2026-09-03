'use strict'

const nativeBinding = require('../index.js')
const { createBuild } = require('./plugin-adapter.js')

module.exports = nativeBinding
module.exports.build = createBuild(nativeBinding.build, nativeBinding)
