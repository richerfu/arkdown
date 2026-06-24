/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/previewer/copy-preview-profile.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * 处理资源增量编译时，restool不复制resources目录，手动copy profile路由配置文件
 * 此任务要处理好增量，因为会写入res目录，输出会影响以res目录作为输入的任务。
 */
export declare class CopyPreviewProfile extends OhosHapTask {
    private readonly profilePath;
    private readonly targetProfilePath;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    protected doTaskAction(): void;
}
