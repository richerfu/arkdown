/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/syscap-transform.d.ts
 */
import { TargetTaskService } from './service/target-task-service';
import { JsonProfile } from '../model/json-profile';
import { AbstractSyscapTransform } from './abstract/abstract-syscap-transform';
/**
 * Syscap Transform Task
 *
 * @since 2022/02/22
 */
export declare class SyscapTransform extends AbstractSyscapTransform {
    private readonly jsonFilePath;
    constructor(taskService: TargetTaskService);
    protected getJsonProfileByModel(): JsonProfile;
}
