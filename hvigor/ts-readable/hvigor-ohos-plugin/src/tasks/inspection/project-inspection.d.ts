/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/inspection/project-inspection.d.ts
 */
import { ProjectModel } from '../../model/project/project-model';
import { InspectionI } from './inspection-i';
export declare class ProjectInspection implements InspectionI {
    private _log;
    private readonly projectModel;
    constructor(projectModel: ProjectModel);
    doInspection(): InspectionI;
    projectPathInspection(): void;
    exitOnError(): void;
}
