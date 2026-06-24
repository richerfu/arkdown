/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/packing-check/packing-check.d.ts
 */
import { OhosLogger } from '../../utils/log/ohos-logger';
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * Hap/Hsp模块在打包前，对包产物进行检查
 *
 */
export declare class PackingCheck extends OhosHapTask {
    readonly _log: OhosLogger;
    private readonly etsAssetsPath;
    private readonly etsOriginalPath;
    private readonly intermediateResourceDir;
    protected readonly targetJsonPath: string;
    private readonly moduleJsonObj;
    constructor(taskService: TargetTaskService);
    taskShouldDo(): boolean;
    protected doTaskAction(): void;
    /**
     * 校验 module.json5 中配置的 srcEntry 对应的 .so 产物是否存在
     *
     */
    private checkSoFilesExist;
    initTaskDepends(): void;
}
