/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/ability/legacy-ability-model.d.ts
 */
import { AbilityTypeEnum } from '../../enum/ability-type-enum';
import { CodeType } from '../../enum/code-type-enum';
import { ConfigJson } from '../../options/configure/config-json-options';
export interface LegacyAbilityModel {
    /**
     * 返回ability的名称
     */
    getName(): string;
    /**
     * 返回ability的输入/输出的目录名
     */
    getRelateSrcPath(): string;
    /**
     * 返回ability的类型
     */
    getType(): AbilityTypeEnum;
    /**
     * 返回ability使用的编译语言
     *
     * 只可CodeType.ets或者CodeType.js
     */
    getSrcLanguage(): CodeType;
    /**
     * 返回ability对应的js标签
     */
    getConfigJsonJsObj(): ConfigJson.JsObj | undefined;
}
