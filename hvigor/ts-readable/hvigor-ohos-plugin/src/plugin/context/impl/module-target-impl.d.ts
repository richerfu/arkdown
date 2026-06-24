/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/context/impl/module-target-impl.d.ts
 */
import { Product, Target } from '../plugin-context';
import { ModulePathInfoIml } from '../../../common/iml/module-path-info-iml';
import { ModuleBuildProfile } from '../../../options/build/module-build-profile';
import ModuleTargetBuildOpt = ModuleBuildProfile.ModuleTargetBuildOpt;
import { ModuleModel } from '../../../model/module/module-model';
import { ProjectModel } from '../../../model/project/project-model';
import { BuildOption } from '../../../options/build/hap-ohos-config';
import { PathDetails } from "../../../external/path/path-details";
/**
 * Module级别的上下文信息
 */
export declare class ModuleTargetImpl implements Target {
    private readonly targetInfo;
    private readonly modulePathInfo;
    private readonly projectPath;
    private readonly projectModel;
    private readonly moduleModel;
    private currentProduct;
    private buildTargetOutputPath;
    constructor(targetInfo: ModuleTargetBuildOpt, modulePathInfo: ModulePathInfoIml, projectModel: ProjectModel, moduleModel: ModuleModel);
    getBuildTargetOutputPath(): string;
    getTargetName(): string;
    getCurrentProduct(): Product;
    getBuildOption(): BuildOption;
    getModulePathDetails(): PathDetails;
    getBuildTargetOutputPathInternal(): string;
    getTargetNameInternal(): string;
    getCurrentProductInternal(): Product;
    getBuildOptionInternal(): BuildOption;
    getModulePathDetailsInternal(): PathDetails;
    private _getBuildTargetOutputPath;
    private _getTargetName;
    private _getCurrentProduct;
    private _getBuildOption;
    private _getModulePathDetails;
    initCurrentProduct(): void;
    initBuildPath(): void;
}
