/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/configure/restool-config-options.d.ts
 */
export interface ResToolConfigObj {
    configPath?: string;
    packageName?: string;
    output?: string;
    startId?: string;
    moduleNames?: string;
    ResourceTable: string[];
    applicationResource?: string;
    moduleResources: string[];
    dependencies: string[];
    entryCompiledResource?: string;
    ids?: string;
    definedIds?: string;
    iconCheck?: boolean;
    definedSysIds?: string;
    compression?: string;
    thread?: number;
    ignoreResourcePattern?: string[];
    qualifiersConfig?: ResQualifiersConfig;
}
/**
 * 生成restool 链接命令所需参数封装对象
 *
 * @since 2022/9/26
 */
export interface RestoolLinkParamObj {
    appScopeResources?: string;
    moduleResourcesMap?: Map<string, string>;
    harResourcesMap?: Map<string, string>;
    outputPath?: string;
    linkCommand?: string[];
    idsMapPath?: string;
}
export interface ResQualifiersConfig {
    MccMnc?: string[];
    Locale?: string[];
    Orientation?: string[];
    Device?: string[];
    ColorMode?: string[];
    Density?: string[];
}
