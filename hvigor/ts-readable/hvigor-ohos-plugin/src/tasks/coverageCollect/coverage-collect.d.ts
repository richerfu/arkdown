/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/coverageCollect/coverage-collect.d.ts
 */
import { DefaultTask, HvigorCoreNode } from '@ohos/hvigor';
import { TaskService } from '../service/task-service';
/**
 * hvigor coverageCollect命令 任务
 *
 * @since 2023/8/25
 */
export declare class CoverageCollect extends DefaultTask {
    private readonly projectPath;
    private readonly reportPath;
    private readonly coverageFile;
    private readonly _taskService;
    constructor(node: HvigorCoreNode, taskService: TaskService);
    registryAction(): Function;
}
