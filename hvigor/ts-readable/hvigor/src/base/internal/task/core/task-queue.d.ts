/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/task-queue.d.ts
 */
export declare class TaskQueue {
    private taskQueue;
    constructor(capacity?: number);
    push(task: string): boolean;
    pop(): string | undefined;
    peek(): string | undefined;
    size(): number;
    toString(): string;
    isEmpty(): boolean;
    clear(): void;
    popPriorTask(priorTasks: Set<string>): string | undefined;
}
