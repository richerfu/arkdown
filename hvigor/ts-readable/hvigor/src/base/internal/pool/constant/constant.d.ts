/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/constant/constant.d.ts
 */
export declare class PoolConstant {
    static readonly MAX_POOL_NUM: number;
    static readonly MIN_POOL_NUM: number;
    static readonly DEFAULT_MAX_IDLE_TIME: number;
    static readonly DEFAULT_RECYCLE_INTERVAL: number;
    static readonly WORKER_ACTION_PATH: string;
    static readonly WORK_DONE: string;
    static readonly WORK_ERROR: string;
    static readonly CALLBACK_ERROR: string;
    static readonly FILE_SUFFIX: string;
    static readonly UNKNOWN_LOG_TIME: string;
    static readonly ENCODING: string;
    static readonly TIME_PREFIX_LENGTH: number;
    static readonly CPU_LIMIT: number;
    static readonly FAIL_DISPATCH_INTERVAL: number;
    static readonly MAX_FAIL_ATTEMPTS: number;
    static readonly TSC_COMMON_CACHE_KEY = "common";
}
export declare enum WorkerEvent {
    CLEAN_HVIGOR_CONFIG_LOADER = "CLEAN_HVIGOR_CONFIG_LOADER",
    CLEAN_HVIGOR_LOGGER = "CLEAN_HVIGOR_LOGGER",
    UNMOUNT_TSC_COMMON_CACHE_EVENT = "UNMOUNT_TSC_COMMON_CACHE_EVENT",
    UPDATE_LOG_LEVEL = "UPDATE_LOG_LEVEL",
    CUSTOM_WORKER_REQUEST = "CUSTOM_WORKER_REQUEST"
}
