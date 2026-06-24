/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/const/path-const.js
 */
import os_1 from "os";
import path_1 from "path";
import * as hvigor_user_home_js_1 from "../util/hvigor-user-home";
import * as const_js_1 from "./const";
export const DEFAULT_HVIGOR_USER_HOME_CACHE_DIR = path_1.resolve(os_1.homedir(), const_js_1.HVIGOR_USER_HOME_DIR_NAME);
export const HVIGOR_USER_HOME = (0, hvigor_user_home_js_1.getHvigorUserHomeCacheDir)();
export const HVIGOR_WRAPPER_TOOLS_HOME = path_1.resolve(HVIGOR_USER_HOME, "wrapper", "tools");
export const HVIGOR_WRAPPER_PNPM_SCRIPT_PATH = path_1.resolve(HVIGOR_WRAPPER_TOOLS_HOME, "node_modules", ".bin", const_js_1.PNPM_TOOL);
export const HVIGOR_PNPM_STORE_PATH = path_1.resolve(HVIGOR_USER_HOME, "caches");
export const HVIGOR_PROJECT_CACHES_HOME = path_1.resolve(HVIGOR_USER_HOME, const_js_1.PROJECT_CACHES);
export const HVIGOR_PROJECT_ROOT_DIR = process.cwd();
export const HVIGOR_PROJECT_WRAPPER_HOME = path_1.resolve(HVIGOR_PROJECT_ROOT_DIR, const_js_1.HVIGOR);
export const HVIGOR_CONFIG_SCHEMA_PATH = path_1.resolve(__dirname, "../../../res/hvigor-config-schema.json");
export const HVIGOR_PROJECT_CACHE_PATH = path_1.resolve(HVIGOR_PROJECT_ROOT_DIR, const_js_1.HVIGOR_PROJECT_DIR_NAME);
