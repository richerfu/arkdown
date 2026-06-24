/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/manager/resource-task-initializer.d.ts
 */
import { CompileResource } from '../compile-resource';
import { ProcessResourceIncrement } from '../process-resource-increment';
import { ProcessResource } from '../process-resource';
import { TargetTaskService } from '../service/target-task-service';
import { TargetTaskCreator, TaskCreatorManager } from '../task-creator';
export declare const initResourceTasks: (creatorManager: TaskCreatorManager, service: TargetTaskService) => void;
export declare class ProcessResourceCA extends TargetTaskCreator {
    declareTaskDetail: () => import("@ohos/hvigor").TaskDetails;
    declareDepends(): string[];
    provider: () => ProcessResource;
}
export declare class CompileResourceCA extends TargetTaskCreator {
    declareTaskDetail: () => import("@ohos/hvigor").TaskDetails;
    declareDepends(): string[];
    provider: () => CompileResource;
}
export declare class ProcessResourceIncrementCA extends TargetTaskCreator {
    declareDepends: () => string[];
    declareTaskDetail: () => import("@ohos/hvigor").TaskDetails;
    provider: () => ProcessResourceIncrement;
}
