/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generate-metadata.d.ts
 */
import { ModuleBuildProfile } from '../options/build/module-build-profile';
import { AbstractGenerateMetadata } from './abstract/abstract-generate-metadata';
import { ModuleTargetData } from './data/hap-task-target-data';
import { TargetTaskService } from './service/target-task-service';
/**
 * Generate OutputMetadata Task
 */
export declare class GenerateMetadata extends AbstractGenerateMetadata {
    private _hspDepsTarget;
    constructor(targetService: TargetTaskService);
    private get jsonPath();
    private get relatedEntryJsonList();
    protected initTaskDependsForOtherModule(): void;
    protected getDistroFilterObj(relatedEntryTargetData: ModuleTargetData): ModuleBuildProfile.DistroFilterBuildOpt | undefined;
    initTaskDepends(): void;
}
