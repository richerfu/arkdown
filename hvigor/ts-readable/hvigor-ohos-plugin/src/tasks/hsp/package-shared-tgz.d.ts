/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hsp/package-shared-tgz.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
export declare class PackageSharedTgz extends OhosHapTask {
    private readonly _taskService;
    private readonly outputDir;
    private readonly RELEASE;
    private readonly isRelease;
    private readonly tempSharedHspPath;
    private readonly tempSharedHarPath;
    private readonly tgzOutPath;
    private readonly sharedHspName;
    private readonly sharedHspRename;
    private readonly sharedHarName;
    private readonly sharedHspPath;
    private readonly sharedHarPath;
    private readonly _log;
    constructor(taskService: TargetTaskService);
    private get generateSharedTgz();
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    initTaskDepends(): void;
    protected beforeTask(): void;
    taskShouldDo(): boolean;
    /**
     * 只在release模式下执行tgz打包操作
     * @protected
     */
    protected doTaskAction(): Promise<void>;
    private buildTgzPackage;
    private copyTempFiles;
    private removeTempFiles;
}
