/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/util/task-input-value-entry.d.ts
 */
export declare class ValueEntry {
    private readonly _name;
    private readonly _value;
    private readonly _valueType;
    private _hash;
    constructor(name: string, value: TaskInputValue);
    getName(): string;
    getValue(): TaskInputValue;
    getValueType(): unknown;
    getHash(): string;
}
export type TaskInputValue = number | string | boolean | string[] | number[] | boolean[];
