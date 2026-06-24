import { instantiateNapiModuleSync, MessageHandler, WASI } from '@napi-rs/wasm-runtime'

function emitWasiOutput(stream, args) {
  globalThis.postMessage({
    type: 'arkdown:wasi-output',
    stream,
    message: Array.from(args, String).join(' '),
  })
}

const handler = new MessageHandler({
  onLoad({ wasmModule, wasmMemory }) {
    const wasi = new WASI({
      print: function () {
        emitWasiOutput('stdout', arguments)
      },
      printErr: function() {
        emitWasiOutput('stderr', arguments)
      },
    })
    return instantiateNapiModuleSync(wasmModule, {
      childThread: true,
      wasi,
      overwriteImports(importObject) {
        importObject.env = {
          ...importObject.env,
          ...importObject.napi,
          ...importObject.emnapi,
          memory: wasmMemory,
        }
      },
    })
  },
})

globalThis.onmessage = function (e) {
  handler.handle(e)
}
