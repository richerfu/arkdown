/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/global/core-project-profile.d.ts
 */
import { ProjectBuildProfile } from '../../options/build/project-build-profile';
import ProjectProfileOpt = ProjectBuildProfile.ProjectProfileOpt;
import ProductBuildOpt = ProjectBuildProfile.ProductBuildOpt;
export declare class CoreProjectProfile {
    private profileOpt;
    private readonly projectPath;
    private static coreProjectProfile;
    private constructor();
    static getInstance(projectPath: string): CoreProjectProfile;
    private loadProfile;
    getProjectProfileOpt(): ProjectProfileOpt;
    getCurrentProductOpt(): ProductBuildOpt;
}
