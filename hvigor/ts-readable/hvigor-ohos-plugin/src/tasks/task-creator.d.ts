/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/task-creator.d.ts
 */
import { CoreTask, Module, TaskDetails, TaskCreation, TaskContainer } from '@ohos/hvigor';
import { AbstractModulePlugin } from '../plugin/common/abstract-module-plugin';
import { ModuleTaskService } from './service/module-task-service';
import { TargetTaskService } from './service/target-task-service';
export interface CreationConfiguration {
    declareTaskDetail(): TaskDetails;
    declareDepends(): string[];
    configure(): void;
}
export declare abstract class DefaultTaskCreator implements CreationConfiguration, TaskCreation {
    abstract configure: () => void;
    abstract afterConfigure: () => void;
    abstract finalConfigured: () => void;
    abstract declareDepends(): string[];
    abstract declareTaskDetail(): TaskDetails;
    abstract depends: string[];
    abstract dependsDefinedByUser: string[];
    abstract details: TaskDetails;
    abstract provider: () => CoreTask;
}
export interface TargetAwareTask {
    targetService: TargetTaskService;
}
export declare abstract class GlobalTaskCreator extends DefaultTaskCreator {
    protected plugin: AbstractModulePlugin;
    protected service: ModuleTaskService;
    protected isFa: boolean;
    depends: string[];
    dependsDefinedByUser: string[];
    details: TaskDetails;
    constructor(service: ModuleTaskService, plugin: AbstractModulePlugin, isFa?: boolean);
    configure: () => void;
    afterConfigure: () => void;
    finalConfigured: () => void;
}
export declare abstract class TargetTaskCreator extends DefaultTaskCreator implements TargetAwareTask {
    protected node: Module;
    protected isFa: boolean;
    protected isFastBuild: boolean;
    targetService: TargetTaskService;
    depends: string[];
    dependsDefinedByUser: string[];
    details: TaskDetails;
    constructor(service: TargetTaskService, isFa?: boolean);
    configure: () => void;
    afterConfigure: () => void;
    finalConfigured: () => void;
}
export declare class TaskCreatorManager {
    private readonly _taskContainer;
    private readonly _creators;
    getTaskContainer(): TaskContainer;
    constructor(taskContainer: TaskContainer);
    registry(creator: DefaultTaskCreator): void;
    configure(): void;
}
