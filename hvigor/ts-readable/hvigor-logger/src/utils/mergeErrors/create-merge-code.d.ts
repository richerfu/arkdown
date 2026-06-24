/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/create-merge-code.d.ts
 */
import { MergeTwoProperty } from '../../interfaces/merge-two-property';
import { CreateMergePropertyFactory } from './create-merge-property-factory';
export declare class CreateMergeCode implements CreateMergePropertyFactory {
    newMergeProperty(): MergeTwoProperty;
}
