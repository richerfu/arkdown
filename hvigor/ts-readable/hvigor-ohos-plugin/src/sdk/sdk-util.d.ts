/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/sdk-util.d.ts
 */
import { Component, OhLocalComponentLoader, PathAndApiVersion, SdkProxyInfo } from '@ohos/sdkmanager-common';
import { ProjectBuildProfile } from '../options/build/project-build-profile';
/**
 * 根据OpenHarmony sdk目录结构规则预测是否存在相关组件
 * /${OhosSdkRoot}/${API}/${component}
 * /sdk/openharmony/10/toolchains
 *
 * @param sdkDir
 * @param version
 * @param components
 */
export declare const ohosPredict: (sdkDir: string, version: string, components: string[]) => Map<PathAndApiVersion, Component>;
/**
 * 复用sdk-manager的oh-uni-package.json的解析能力
 */
export declare class OhosParser extends OhLocalComponentLoader {
    constructor(sdkRoot: string);
    parse(packages: string[]): Component[];
}
/**
 * 解析compileSdkVersion，compatibleSdkVersion，targetSdkVersion为ApiMeta对象
 *
 * @param sdkVersion 用户在build-profile里配置的compileSdkVersion，compatibleSdkVersion，targetSdkVersion
 * @param isHarmonyOS 判断是否为HarmonyOS项目
 */
export declare const parseApiVersion: (sdkVersion: string | number, isHarmonyOS: boolean) => ProjectBuildProfile.ApiMeta;
export declare const proxyFun: () => SdkProxyInfo;
export declare const contains: (pathAndApi: string, all: Map<string, Component>) => Component | undefined;
export declare const handleSdkException: (ex: any) => void;
