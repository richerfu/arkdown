/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/create/legacy-task-initializer.d.ts
 */
import { Module } from '@ohos/hvigor';
import { AbstractModulePlugin } from '../../../plugin/common/abstract-module-plugin';
import { AbstractModuleHookTask } from '../../hook/abstract-module-hook-task';
import { ModuleTaskService } from '../../service/module-task-service';
import { TargetTaskService } from '../../service/target-task-service';
export declare const legacyHapTasksInitialize: (plugin: AbstractModulePlugin) => void;
export declare const legacyHarTasksInitialize: (plugin: AbstractModulePlugin) => void;
export declare const legacyCommonHooksInitialize: (moduleTaskService: ModuleTaskService) => AbstractModuleHookTask[];
export declare const legacyTasksInitialize: (module: Module, service: TargetTaskService) => void;
