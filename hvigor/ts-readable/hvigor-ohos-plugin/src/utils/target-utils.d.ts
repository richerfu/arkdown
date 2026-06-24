/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/target-utils.d.ts
 */
import { ProjectModel } from '../model/project/project-model';
import { ModuleBuildProfile } from '../options/build/module-build-profile';
export declare class TargetUtils {
    static pushTryTargets(params: {
        projectModel: ProjectModel;
        moduleName: string;
        superTargetName: string;
    }): string[];
    static computeTargets(specificTargets: any, optionTargets: ModuleBuildProfile.ModuleTargetBuildOpt[]): string[];
    static getFallbackTargets(): string[];
}
