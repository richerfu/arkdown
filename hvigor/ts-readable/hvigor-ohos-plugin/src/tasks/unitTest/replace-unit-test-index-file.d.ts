/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/replace-unit-test-index-file.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * 单元测试 stage模型中替换的入口文件
 */
export declare class ReplaceUnitTestIndexFile extends OhosHapTask {
    private _log;
    private PAGE_JSON;
    private PAGE_JSON_FILE;
    private PAGE_JSON_RESOURCE;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    protected doTaskAction(): void;
    private writeModuleJson;
    private writeMainPages;
}
