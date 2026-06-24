/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/project-config/log-info.d.ts
 */
import { Level } from 'log4js';
/**
 * 日志相关的字段
 */
export interface LogInfo {
    /**
     * 日志的级别
     */
    level?: Level;
}
