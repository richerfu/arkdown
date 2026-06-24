/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/task/impl/task-impl.d.ts
 */
import { CoreTask } from '../../../external/task/core-task';
import { Task } from '../interface/task';
/**
 * 对外提供的Task视图接口实现
 */
export declare class TaskImpl implements Task {
    private readonly task;
    constructor(task: CoreTask);
    getDependencies(): string[];
    getDependenciesInternal(): string[];
    private _getDependencies;
    getName(): string;
    getNameInternal(): string;
    private _getName;
    setEnable(enable: boolean): void;
    setEnableInternal(enable: boolean): void;
    private _setEnable;
    beforeRun(fn: Function): void;
    beforeRunInternal(fn: Function): void;
    private _beforeRun;
    afterRun(fn: Function): void;
    afterRunInternal(fn: Function): void;
    private _afterRun;
}
