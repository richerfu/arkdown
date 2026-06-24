/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/trace/json-trace.d.ts
 */
import { JSON_VALUE_RECORD } from './type';
export declare class JsonTrace {
    private readonly telemetry;
    constructor(telemetryPath: string);
    /**
     * 获取构建选项的追踪记录
     * @param json - 需要记录的对象
     * @returns 返回BUILD_OPTION_RECORD数组
     */
    getRecord(json: unknown): JSON_VALUE_RECORD[];
    private collectRecordRecursively;
    private processCurrentNode;
    private processChildNodes;
}
