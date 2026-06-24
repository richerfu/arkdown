/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/project/arkui-x/crossplatform-interface.d.ts
 */
import { ArkUIXConfigJson } from '../../../options/configure/arkui-x-config-opt';
export interface CrossplatformInterface {
    /**
     * 对外暴露判断当前工程是否为跨平台Arkui-x的工程
     */
    isCrossplatformProject(): boolean;
    /**
     * 获取Arkui-x工程配置文件arkui-x-config.json5路径
     */
    getArkUIXConfigJsonPath(): string;
    /**
     * 获取Arkui-x工程配置文件arkui-x-config.json5反序列化的对象
     */
    getArkUIXConfigJsonObj(): ArkUIXConfigJson.ConfigObj | undefined;
}
