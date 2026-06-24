/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-integrated-hsp.d.ts
 */
import { TargetTaskService } from './service/target-task-service';
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { OhosLogger } from '../utils/log/ohos-logger';
import { OhosModuleTask } from './task/ohos-module-task';
export declare class ProcessIntegratedHsp extends OhosModuleTask {
    initTaskDepends(): void;
    protected readonly _log: OhosLogger;
    protected readonly inputFile: string;
    protected readonly integratedRemoteHspDir: string;
    private readonly allRemoteHspPathMap;
    private readonly integratedInputList;
    private readonly integratedOutputList;
    constructor(taskService: TargetTaskService);
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    protected doTaskAction(): Promise<void>;
    /**
     * 调用packageTool生成integratedHsp回填后产物
     *
     * @param inputFilePath
     * @param integratedHspPath
     * @private
     */
    private generateIntegratedHsp;
}
