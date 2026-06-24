/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/manager/creation-util.d.ts
 */
import { TaskDetails } from '@ohos/hvigor';
import { ModuleType } from '../../enum/module-type-enum';
import { TargetTaskService } from '../service/target-task-service';
export declare const computeTaskDetailsWithTarget: (details: TaskDetails, service: TargetTaskService) => TaskDetails;
export declare const computeDepends: (depends: string[], service: TargetTaskService) => string[];
export declare const deviceTestComputeDepends: (depends: string[], depTask: string, service: TargetTaskService, moduleType: ModuleType) => void;
export declare const computeHookTaskDepends: (depends: string[], targets: TargetTaskService[]) => string[];
