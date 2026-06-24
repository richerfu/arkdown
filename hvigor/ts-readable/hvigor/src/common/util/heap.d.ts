/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/heap.d.ts
 */
export declare class Heap<T> {
    private elements;
    private element2index;
    private size;
    private readonly compare;
    constructor(compare: (a: T, b: T) => number);
    update(x: T): void;
    clear(): void;
    peek(): T | undefined;
    push(x: T): void;
    pop(): T | undefined;
    empty(): boolean;
    private up;
    private down;
    private swap;
}
