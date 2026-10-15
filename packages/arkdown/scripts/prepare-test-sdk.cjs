'use strict'

const fs = require('node:fs')
const path = require('node:path')

class TestSdkFixture {
  install(sdkHome) {
    if (!sdkHome) throw new Error('OHOS_SDK_HOME must point to the downloaded test SDK')
    const sdk = fs.realpathSync(sdkHome)
    if (!fs.statSync(path.join(sdk, 'ets/build-tools/ets-loader')).isDirectory()) {
      throw new Error(`Missing test SDK ets-loader: ${sdk}`)
    }
    // SdkComponentInventory discovers external components beside the selected
    // SDK root. Keep the public SDK files intact and install our own component.
    const destination = path.join(path.dirname(sdk), 'arkdown-ci-sdk-checker')
    fs.cpSync(path.resolve(__dirname, '../__test__/fixtures/sdk-checker'), destination, { recursive: true })
    return destination
  }
}

module.exports = { TestSdkFixture }

if (require.main === module) {
  new TestSdkFixture().install(process.env.OHOS_SDK_HOME)
}
