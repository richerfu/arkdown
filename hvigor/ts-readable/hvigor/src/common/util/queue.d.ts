/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/queue.d.ts
 */
export declare class Queue<T> {
    private elements;
    private readonly _capacity;
    constructor(capacity?: number);
    push(element: T): boolean;
    pop(): T | undefined;
    size(): number;
    peek(): T | undefined;
    clear(): void;
    /**
     * 如果存在则移除
     * @param search
     */
    popPriorTask(priorTasks: Set<T>): T | undefined;
    toString(): string;
}
