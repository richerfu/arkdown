/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/const/daemon-const.js
 */
import path_1 from "path";
import * as const_js_1 from "../../../common/const/const";
import * as path_const_js_1 from "../../../common/const/path-const";
export const DAEMON_LOG_PATH = path_1.resolve(path_const_js_1.HVIGOR_USER_HOME, "daemon", "log", const_js_1.CUR_HVIGOR_VERSION);
export const DAEMON_PROCESS_EXIT_TIME_OUT = 108e5;
export const DAEMON_PROCESS_REMOVED_FROM_REGISTRY_CHECK_TIME = 3e4;
export const DAEMON_SERVER_MIN_LISTEN_PORT = 45e3;
export const DAEMON_SERVER_MAX_LISTEN_PORT = 45099;
export const DEFAULT_DAEMON_REGISTRY_PATH = path_1.resolve(path_const_js_1.HVIGOR_USER_HOME, "daemon", "cache", "daemon-sec.json");
export const DEFAULT_DAEMON_META_PATH = path_1.resolve(path_const_js_1.HVIGOR_USER_HOME, "meta");
export const DAEMON_WORK_KEY_LENGTH = 16;
export const DAEMON_SALT_KEY_LENGTH = 16;
export const DAEMON_ROOT_KEY_COMPONENT_LENGTH = 128;
export const KEY_FILE_PERMISSIONS = 384;
export const KEY_FILE_DIRECTORY_PERMISSIONS = 448;
export const DAEMON_SERVER_LOCAL_HOST = "127.0.0.1";
export const DAEMON_PROCESS_ACTIVE_MAX_LIMIT = 6;
export const DAEMON_INFO_IN_REGISTRY_MAX_LIMIT = 8;
export const DAEMON_SOCKETS_MAX_LIMIT = 10;
export const DAEMON_LOG_FILE_NAME_PATTEN = /^(daemon-[0-9]+\.log)(\.[0-9]+\.gz)?$/;
export const DAEMON_LOG_FILE_RETENTION_TIME = 864e5;
export const DAEMON_LISTEN_FAILED_REASON_PREFIX = "LISTEN_ERROR:";
