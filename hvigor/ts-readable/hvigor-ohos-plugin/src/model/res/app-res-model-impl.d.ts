/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/res/app-res-model-impl.d.ts
 */
import { AppJson } from '../../options/configure/app-json-options';
import { CoreResModelImpl } from './core-res-model-impl';
import { AppRes } from './res-model';
/**
 * Stage模型的App级别的资源
 *
 * @since 2022/2/23
 */
export declare class AppResModelImpl extends CoreResModelImpl implements AppRes {
    constructor(sourceSetRoot: string);
    getAppResOpt(): AppJson.AppOptObj;
}
