/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-syscap-transform.d.ts
 */
import { AbstractSyscapTransform } from '../abstract/abstract-syscap-transform';
import { TargetTaskService } from '../service/target-task-service';
import { JsonProfile } from '../../model/json-profile';
/**
 * Legacy Syscap Transform Task
 *
 * @since 2022/09/8
 */
export declare class LegacySyscapTransform extends AbstractSyscapTransform {
    private readonly jsonFilePath;
    constructor(taskService: TargetTaskService);
    protected getJsonProfileByModel(): JsonProfile;
}
