/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-generate-metadata.d.ts
 */
import { ModuleBuildProfile } from '../../options/build/module-build-profile';
import { AbstractGenerateMetadata } from '../abstract/abstract-generate-metadata';
import { ModuleTargetData } from '../data/hap-task-target-data';
import { TargetTaskService } from '../service/target-task-service';
/**
 * Legacy Generate OutputMetadata Task
 */
export declare class LegacyGenerateMetadata extends AbstractGenerateMetadata {
    private readonly configJsonPath;
    private get relatedEntryJsonList();
    constructor(targetService: TargetTaskService);
    protected getDistroFilterObj(relatedEntryTargetData: ModuleTargetData): ModuleBuildProfile.DistroFilterBuildOpt | undefined;
}
