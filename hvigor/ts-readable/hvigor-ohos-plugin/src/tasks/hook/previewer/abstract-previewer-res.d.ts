/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/previewer/abstract-previewer-res.d.ts
 */
import { ModuleTaskService } from '../../service/module-task-service';
import { AbstractModuleHookTask } from '../abstract-module-hook-task';
export declare abstract class AbstractPreviewerRes extends AbstractModuleHookTask {
    protected syscapJsonPath: string;
    protected constructor(moduleService: ModuleTaskService, isFaMode: boolean);
}
