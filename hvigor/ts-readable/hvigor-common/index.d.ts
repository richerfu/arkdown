/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/index.d.ts
 */
export { isCI } from './src/util/ci-util';
export { isSubPath, isChildPath } from './src/util/path-util';
export { createHash, hash, hashFile } from './src/util/hash-util';
export { countryEnum, getOsLanguage } from './src/util/os-country-util';
export { isWindows, isMac, isLinux, maxPathLength } from './src/util/system-util';
export { getZipReader, getZipWriter, readFileInZIP, addFilesToZip } from './src/util/zip-util';
export { freezeDeep } from './src/util/object-util';
export { validateDataBySchema } from './src/util/json-schema-util';
export { strMapToObj } from './src/util/type-conversion-util';
export { lockSync, unlockSync, checkSync } from './src/util/file-lock';
export type { LockOptions, LockInfo } from './src/util/file-lock';
export { parseJsonText } from './src/util/parse-json/parse-json-util';
export { mod } from './src/util/math-util';
export { isBuiltinModule } from './src/util/is-builtin-module';
export { traverseFilesSync, type Filter, shouldIncludeFile } from './src/util/file-util';
export { JsonTrace } from './src/trace/json-trace';
export type { JSON_VALUE_RECORD } from './src/trace/type';
export { createPromiseLimit } from './src/util/p-limit-util';
export { HVIGOR_PROJECT_WRAPPER_HOME } from './src/constants/hvigor-user-home-constants';
export { DEFAULT_PROJECT_NODE_PATH } from './src/constants/process-env-constants';
