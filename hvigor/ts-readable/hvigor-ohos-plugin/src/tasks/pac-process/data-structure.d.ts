/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pac-process/data-structure.d.ts
 */
export interface DataLabel {
    label: string;
    purposes: string[];
    userLinked?: boolean;
    tracking?: boolean;
}
export interface DataProcess {
    dataType: string;
    dataLabels: DataLabel[];
}
export interface SpecialAPI {
    apiType: string;
    reasons: string[];
}
export interface PacFile {
    dataProcess?: DataProcess[];
    specialAPIs?: SpecialAPI[];
}
