/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/sign-request.js
 */
import * as websocket_js_1 from "../java-daemon/websocket";
export async function sendSignRequest(e, s) {
  const t = {
    type: websocket_js_1.MessageType.SIGN_APP,
    data: JSON.stringify(s)
  };
  return (0, websocket_js_1.sendSocketRequest)(t, e);
}
export async function sendSignHarRequest(e, s) {
  const t = {
    type: websocket_js_1.MessageType.SIGN_SDK,
    data: JSON.stringify(s)
  };
  return (0, websocket_js_1.sendSocketRequest)(t, e);
}
export async function sendVerifyProfileRequest(e, s) {
  const t = {
    type: websocket_js_1.MessageType.VERIFY_PROFILE,
    data: JSON.stringify(s)
  };
  return (0, websocket_js_1.sendSocketRequest)(t, e);
}
export function constructSignParams(e, s, t, r, i, n, a = "localSign") {
  return {
    mode: a,
    keystoreFile: e.material.storeFile,
    keystorePwd: r,
    keyAlias: e.material.keyAlias,
    keyPwd: i,
    signAlg: e.material.signAlg,
    profileFile: e.material.profile,
    appCertFile: e.material.certpath,
    inForm: n,
    inFile: s,
    outFile: t
  };
}
export function constructHarSignParams(e, s, t, r, i) {
  return {
    keystoreFile: e.material.storeFile,
    keystorePwd: r,
    keyAlias: e.material.keyAlias,
    keyPwd: i,
    inFileForm: "DIR",
    signAlg: e.material.signAlg,
    profileFile: e.material.profile,
    signCertFile: e.material.certpath,
    inFile: s
  };
}
