/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/project/arkui-x/arkui-x-project-model.d.ts
 */
import { ArkUIXConfigJson } from '../../../options/configure/arkui-x-config-opt';
import { ProjectBuildProfile } from '../../../options/build/project-build-profile';
import { CrossplatformInterface } from './crossplatform-interface';
export declare class ArkUIXProjectModel implements CrossplatformInterface {
    private readonly projectName;
    private readonly projectPath;
    private readonly profileOptions;
    private readonly _log;
    protected _arkUIXConfigJsonObj: ArkUIXConfigJson.ConfigObj | undefined;
    constructor(projectName: string, projectPath: string, profileOptions: ProjectBuildProfile.ProjectProfileOpt);
    isCrossplatformProject(): boolean;
    getArkUIXConfigJsonPath(): string;
    getArkUIXConfigJsonObj(): ArkUIXConfigJson.ConfigObj | undefined;
    private initArkUIXConfigJsonObj;
    private arkUIXModuleCheck;
}
