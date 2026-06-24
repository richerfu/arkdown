/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/iml/project-path-info-iml.d.ts
 */
import { ProjectModel } from '../../model/project/project-model';
export declare class ProjectPathInfoIml {
    private readonly _projectModel;
    private readonly _productName;
    private readonly _projectPath;
    constructor(projectModel: ProjectModel, productName: string);
    getProjectBuildPath(): string;
    getProjectOutputPath(): string;
    getProjectDebugSymbolPath(): string;
}
