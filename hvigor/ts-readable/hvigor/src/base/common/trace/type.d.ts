/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/trace/type.d.ts
 */
import { MoreInfo } from '@ohos/hvigor-logger';
/**
 * hvigor项目的打点数据声明
 */
export interface HvigorTraceData {
    TOTAL_TIME?: number;
    IS_INCREMENTAL: boolean;
    IS_DAEMON: boolean;
    IS_PARALLEL: boolean;
    IS_HVIGORFILE_TYPE_CHECK: boolean;
    IS_EXECUTION_OHPM_BY_HVIGOR: boolean;
    BUILD_ANALYSIS_MODE: string;
    ERROR_MESSAGE?: {
        CODE?: string;
        TIMESTAMP?: string;
        BUILD_ID?: string;
        MESSAGE?: string;
        SOLUTIONS?: string[];
        MORE_INFO?: MoreInfo;
        COMPONENTS?: string;
        CHECK_MESSAGE?: string;
        STACK?: string;
        PROJECT_CONFIG?: {
            [k: string]: any;
        };
    }[];
    TASK_TIME?: Record<string, Record<string, number>>;
    CUSTOM_TASK_TIME?: Record<string, Record<string, number>>;
    CUSTOM_PLUGIN_TIME: Record<string, number>;
    BUILD_ID?: string;
    APIS: Set<string>;
    CONFIG_PROPERTIES?: {
        [key: string]: any;
    };
    CONFIG_EXPERIMENT?: {
        [key: string]: any;
    };
}
