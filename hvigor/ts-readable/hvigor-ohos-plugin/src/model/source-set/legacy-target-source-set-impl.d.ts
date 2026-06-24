/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/source-set/legacy-target-source-set-impl.d.ts
 */
import { LegacyModuleTargetRes } from '../res/res-model';
import { CoreSourceSetImpl } from './core-source-set-impl';
import { LegacyTargetSourceSetModel } from './source-set-model';
export declare class LegacyTargetSourceSetImpl extends CoreSourceSetImpl implements LegacyTargetSourceSetModel {
    constructor(sourceSetRoot: string);
    getLegacyModuleTargetRes(): LegacyModuleTargetRes;
    getTargetSourceSetModuleName(): string | undefined;
}
