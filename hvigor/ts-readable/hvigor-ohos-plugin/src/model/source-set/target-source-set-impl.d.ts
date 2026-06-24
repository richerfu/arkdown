/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/source-set/target-source-set-impl.d.ts
 */
import { ModuleTargetRes } from '../res/res-model';
import { CoreSourceSetImpl } from './core-source-set-impl';
import { TargetSourceSetModel } from './source-set-model';
/**
 * module中每个target对应的一个sourceSet对象
 *
 */
export declare class TargetSourceSetImpl extends CoreSourceSetImpl implements TargetSourceSetModel {
    constructor(sourceSetRoot: string);
    getModuleTargetRes(): ModuleTargetRes;
    getTargetSourceSetModuleName(): string | undefined;
}
