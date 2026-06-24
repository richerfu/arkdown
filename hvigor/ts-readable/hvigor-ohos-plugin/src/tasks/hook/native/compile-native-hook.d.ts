/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/native/compile-native-hook.d.ts
 */
import { ModuleTaskService } from '../../service/module-task-service';
import { TargetTaskService } from '../../service/target-task-service';
import { AbstractModuleHookTask } from '../abstract-module-hook-task';
/**
 * 暴露给其他模块使用的hook任务,任务名不可更改
 *
 * @since 2022/5/13
 */
export declare class CompileNativeHook extends AbstractModuleHookTask {
    constructor(moduleService: ModuleTaskService);
    initTaskDepends(taskTargetService: TargetTaskService): void;
}
