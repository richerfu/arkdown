/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/inspection/module-inspection.d.ts
 */
import { ModuleModel } from '../../model/module/module-model';
import { TargetRuntimeOS } from '../../options/build/module-build-profile';
import { ProjectBuildProfile } from '../../options/build/project-build-profile';
import { InspectionI } from './inspection-i';
import ProductBuildOpt = ProjectBuildProfile.ProductBuildOpt;
export declare class ModuleInspection implements InspectionI {
    private _log;
    private readonly projectModel;
    private readonly moduleModel;
    constructor(moduleModel: ModuleModel);
    doInspection(): InspectionI;
    /**
     * 收集各个模块target的runtimeOS信息
     * 用于runtimeOS及API校验
     */
    collectTargetRuntimeOS(): void;
    targetApiVersionCheck(): void;
    targetBuildOptInspection(): void;
    runtimeOSInspection(target: TargetRuntimeOS, product: ProductBuildOpt): void;
    private checkModuleType;
    exitOnError(): void;
}
