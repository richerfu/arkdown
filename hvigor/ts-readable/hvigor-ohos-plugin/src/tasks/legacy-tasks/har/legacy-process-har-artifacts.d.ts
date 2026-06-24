/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/har/legacy-process-har-artifacts.d.ts
 */
import { AbstractProcessHarArtifacts } from '../../abstract/abstract-process-har-artifacts';
import { TargetTaskService } from '../../service/target-task-service';
import { OhosLogger } from '../../../utils/log/ohos-logger';
/**
 * 收集FA模型打包需要用的构建产物
 */
export declare class LegacyProcessHarArtifacts extends AbstractProcessHarArtifacts {
    protected logger: OhosLogger;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    copyCompiledSourceFileToTempDir(): void;
    copyDeclareFileToTempDir(): void;
    copyOtherGenerateFiles(): void;
    processPackageJson(): void;
    processSourceMaps(): void;
    addPreloadSystemSoJson(): void;
}
