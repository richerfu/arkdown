'use strict'

const nativeBinding = require('./index.js')
const { createBuild } = require('./runtime.js')

module.exports = nativeBinding
module.exports.build = createBuild(nativeBinding.build)
