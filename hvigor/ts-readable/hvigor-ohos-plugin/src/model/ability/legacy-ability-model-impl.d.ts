/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/ability/legacy-ability-model-impl.d.ts
 */
import { AbilityTypeEnum } from '../../enum/ability-type-enum';
import { CodeType } from '../../enum/code-type-enum';
import { ConfigJson } from '../../options/configure/config-json-options';
import { LegacyAbilityModel } from './legacy-ability-model';
/**
 * Fa模型中的ability数据模型
 *
 * @since 2022/1/19
 */
export declare class LegacyAbilityModelImpl implements LegacyAbilityModel {
    private _log;
    private readonly _configObj;
    private readonly _configFile;
    private readonly _abilityName;
    private readonly _abilityObj;
    private readonly _jsObj;
    constructor(configJson: string, abilityName: string);
    private findJsByName;
    private hasJsObj;
    private findAbilityByName;
    getRelateSrcPath(): string;
    getName(): string;
    getSrcLanguage(): CodeType;
    getType(): AbilityTypeEnum;
    getConfigJsonJsObj(): ConfigJson.JsObj | undefined;
}
