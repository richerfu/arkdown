/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/task/ohos-hap-task.d.ts
 */
import { TaskDetails } from '@ohos/hvigor';
import { CompileModeEnum } from '../../enum/compile-mode-enum';
import { TargetTaskService } from '../service/target-task-service';
import { OhosModuleTask } from './ohos-module-task';
/**
 * openHarmony基础hap 的task
 *
 * @since 2022/1/5
 */
export declare abstract class OhosHapTask extends OhosModuleTask {
    protected readonly targetCompileMode: CompileModeEnum;
    protected constructor(targetService: TargetTaskService, taskDetails: TaskDetails);
}
