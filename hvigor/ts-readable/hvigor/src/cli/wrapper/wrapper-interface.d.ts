/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/cli/wrapper/wrapper-interface.d.ts
 */
export { PROJECT_CACHES, WORK_SPACE, PNPM_TOOL, HVIGOR, DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME, CUR_HVIGOR_VERSION } from '../../common/const/const';
export type { HvigorConfigJson, PackageJson } from '../../common/type';
export { getHvigorUserHomeCacheDir } from '../../common/util/hvigor-user-home';
export { calcChildExecArgv } from '../../common/util/cacl-exec-argv-util';
export { exit } from '../../base/boot/hooks/exit';
export { ParseJsonFile } from '../../common/util/parse-json-file';
export { HVIGOR_PROJECT_WRAPPER_HOME, HVIGOR_PROJECT_ROOT_DIR, HVIGOR_USER_HOME, HVIGOR_WRAPPER_TOOLS_HOME, HVIGOR_WRAPPER_PNPM_SCRIPT_PATH, HVIGOR_PNPM_STORE_PATH, } from '../../common/const/path-const';
export { calProjectCacheDir } from './util';
