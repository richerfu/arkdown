/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/res/target-res-model-impl.d.ts
 */
import { ModuleJson } from '../../options/configure/module-json-options';
import { CoreResModelImpl } from './core-res-model-impl';
import { ModuleTargetRes } from './res-model';
/**
 * Stage Module 的不同target对应的资源集合
 *
 * @since 2022/2/23
 */
export declare class ModuleTargetResImpl extends CoreResModelImpl implements ModuleTargetRes {
    constructor(sourceSetRoot: string);
    getModuleJsonOpt(): ModuleJson.ModuleOptObj;
    getFormJsonOpt(formPath: string): ModuleJson.FormsObj;
}
