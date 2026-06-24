/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/har/legacy-package-har.d.ts
 */
import { AbstractPackageHar } from '../../abstract/abstract-package-har';
import { TargetTaskService } from '../../service/target-task-service';
/**
 * 打包Fa模型的Har包
 *
 * @since 2021/12/18
 */
export declare class LegacyPackageHar extends AbstractPackageHar {
    protected readonly taskTmpDir: string;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
}
