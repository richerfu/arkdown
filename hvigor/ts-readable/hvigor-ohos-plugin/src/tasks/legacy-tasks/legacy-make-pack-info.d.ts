/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-make-pack-info.d.ts
 */
import { LegacyModuleModelImpl } from '../../model/module/legacy-module-model-impl';
import { ConfigJson } from '../../options/configure/config-json-options';
import { ModuleTargetData } from '../data/hap-task-target-data';
import { TargetTaskService } from '../service/target-task-service';
import { FileSet } from '@ohos/hvigor';
import { AbstractMakePackInfo } from '../abstract-make-pack-info';
/**
 * 生成老模型hap级别的pack.info
 */
export declare class LegacyMakePackInfo extends AbstractMakePackInfo {
    private log;
    declareInputFiles(): FileSet;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    protected doTaskAction(): void;
    getModuleObj(deviceTypes: string[], moduleModel: LegacyModuleModelImpl, apiVersion: ConfigJson.ApiVersionObj | undefined, targetName: string): object;
    protected getPackageObj(targetData: ModuleTargetData, configObj: ConfigJson.ConfigOptObj, noSuffixHapName: string, noSuffixLiteHapName?: string): object[];
}
