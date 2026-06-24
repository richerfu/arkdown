/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/package-har.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { AbstractPackageHar } from '../abstract/abstract-package-har';
import { TargetTaskService } from '../service/target-task-service';
/**
 * 打包Har包
 *
 * @since 2021/12/18
 */
export declare class PackageHar extends AbstractPackageHar {
    protected readonly taskTmpDir: string;
    private readonly _log;
    initTaskDepends(): void;
    constructor(taskService: TargetTaskService);
    private doExpandImportPathCheck;
    beforeAlwaysAction(): Promise<void>;
    declareInputFiles(): FileSet;
    protected doTaskAction(): Promise<void>;
}
