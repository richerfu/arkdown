'use strict'

// Mock only the public callback ABI. Native checking still owns type semantics
// and diagnostics; CI's SDK tools still compile and package the real fixture.
class SyscapChecker {
  check() {
    return { checkResult: true, checkMessage: '' }
  }
}

function apiLevel(version) {
  const text = String(version)
  const value = Number(text.match(/\((\d+)\)$/)?.[1] ?? text)
  if (!Number.isFinite(value)) throw new Error(`Unsupported mock SDK version: ${text}`)
  return value
}

function checkSinceValue(required, compatible) {
  return { result: apiLevel(required) <= apiLevel(compatible), message: '' }
}

module.exports = { SyscapChecker, checkSinceValue }
