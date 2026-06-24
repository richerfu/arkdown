/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/previewer/build-har-previewer-res.d.ts
 */
import { ModuleTaskService } from '../../service/module-task-service';
import { TargetTaskService } from '../../service/target-task-service';
import { AbstractPreviewerRes } from './abstract-previewer-res';
/**
 * for previewer hook only
 *
 * @since 2022/8/11
 */
export declare class BuildHarPreviewerRes extends AbstractPreviewerRes {
    constructor(moduleService: ModuleTaskService, isFaMode: boolean);
    initTaskDepends(taskTargetService: TargetTaskService): void;
}
