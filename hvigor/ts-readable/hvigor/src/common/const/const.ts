/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/const/const.js
 */
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as constant_js_1 from "../../base/internal/pool/constant/constant";
export const HVIGOR_ENGINE_PACKAGE_NAME = "@ohos/hvigor";
export const CUR_HVIGOR_VERSION = require("../../../package.json").version;
export const PNPM_TOOL = (0, hvigor_common_1.isWindows)() ? "pnpm.cmd" : "pnpm";
export const HVIGOR = "hvigor";
export const HVIGOR_PROJECT_DIR_NAME = ".hvigor";
export const DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME = "hvigor-config.json5";
export const DEFAULT_OH_PACKAGE_JSON_FILE_NAME = "oh-package.json5";
export const HVIGOR_USER_HOME_DIR_NAME = ".hvigor";
export const PROJECT_CACHES = "project_caches";
export const WORK_SPACE = "workspace";
export const HVIGOR_CACHE_DIR_KEY = "hvigor.cacheDir";
export const ENABLE_SIGN_TASK_KEY = "enableSignTask";
export const BUILD_CACHE_DIR = "build-cache-dir";
export const HVIGOR_POOL_MAX_SIZE = "hvigor.pool.maxSize";
export const HVIGOR_POOL_MAX_CORE_SIZE = "hvigor.pool.maxCoreSize";
export const HVIGOR_POOL_CACHE_CAPACITY = "hvigor.pool.cache.capacity";
export const HVIGOR_POOL_CACHE_TTL = "hvigor.pool.cache.ttl";
export const OHOS_ARK_COMPILE_MAX_SIZE = "ohos.arkCompile.maxSize";
export const HVIGOR_ENABLE_MEMORY_CACHE = "hvigor.enableMemoryCache";
export const OHOS_ARK_COMPILE_SOURCE_MAP_DIR = "ohos.arkCompile.sourceMapDir";
export const HVIGOR_MEMORY_THRESHOLD = "hvigor.memoryThreshold";
export const PROPERTIES = "properties";
export const DOT = ".";
export const INCREMENTAL_INPUT_OUTPUT_CACHE = "hvigor.incremental.optimization";
export const INCREMENTAL_OPTIMIZATION = "hvigor.task.schedule.optimization";
export const ENABLE_OVERRIDES_DEPENDENCY_MAP = "enableOverridesDependencyMap";
export const DEFAULT_PROJECT_NODE_PATH = process.env.NODE_PATH;
export const ENABLE_SOURCE_MAPS = "--enable-source-maps";
export const ENFORCE_OHPM = "enforce-ohpm";
export const CUSTOM_WORKER_REQUEST = constant_js_1.WorkerEvent.CUSTOM_WORKER_REQUEST;
