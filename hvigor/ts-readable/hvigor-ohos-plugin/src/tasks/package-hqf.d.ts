/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/package-hqf.d.ts
 */
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
export declare class PackageHqf extends OhosHapTask {
    private readonly _log;
    constructor(targetService: TargetTaskService);
    protected doTaskAction(): Promise<void>;
    getHotReloadCommand(): string[];
    getColdReloadCommand(): string[];
    /**
     * 用于根据buildConfig文件里的mode判断重载类型
     * 值：hotReload coldReload
     */
    getPatchConfigMode(): any;
    initTaskDepends(): void;
}
