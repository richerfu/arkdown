/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/ability/legacy-form-model-impl.d.ts
 */
import { AbilityTypeEnum } from '../../enum/ability-type-enum';
import { CodeType } from '../../enum/code-type-enum';
import { ConfigJson } from '../../options/configure/config-json-options';
import { LegacyAbilityModel } from './legacy-ability-model';
/**
 * Fa模型中的form数据模型
 *
 * @since 2022/3/08
 */
export declare class LegacyFormModelImpl implements LegacyAbilityModel {
    private readonly _configObj;
    private readonly _jsName;
    private readonly _jsObj;
    constructor(configJson: string, jsObj: ConfigJson.JsObj);
    getRelateSrcPath(): string;
    getName(): string;
    getSrcLanguage(): CodeType;
    getType(): AbilityTypeEnum;
    getConfigJsonJsObj(): ConfigJson.JsObj | undefined;
}
