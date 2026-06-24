/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/module-meta-info-plugin.d.ts
 */
import { ModuleInfo, PluginContext } from '@ohos/hvigor-arkts-base';
type PackageInfoType = {
    pkgPath: string;
    nodeEntryFiles: string[];
    pkgName?: string;
    pkgVersion?: string;
};
/**
 * 模块化编译场景下，处理软连接问题，给模块加载一些必要的属性信息
 */
export declare function moduleMetaInfoPlugin(extensions: string[]): {
    name: string;
    beforeBuild: typeof beforeBuild;
    buildStart: typeof buildStart;
    load: typeof load;
    moduleParsed: typeof moduleParsed;
    beforeBuildEnd: typeof beforeBuildEnd;
    afterBuildEnd: typeof afterBuildEnd;
    cleanUp: typeof cleanUp;
    resolveDynamicImport: typeof resolveDynamicImport;
    resolveId: typeof resolveId;
};
declare function resolveDynamicImport(this: PluginContext, specifier: string, importer: string): void;
declare function resolveId(this: PluginContext, specifier: string, importer: string): void;
declare function buildStart(this: PluginContext): void;
declare function beforeBuild(this: PluginContext): void;
declare function load(this: PluginContext, id: string): void;
declare function moduleParsed(this: PluginContext, moduleInfo: ModuleInfo): void;
declare function beforeBuildEnd(this: PluginContext): void;
declare function afterBuildEnd(this: PluginContext): void;
declare function cleanUp(): void;
/**
 * 获取oh-package.json/package.json中相关信息，如：入口文件，以及包路径
 *
 * @param filePath 文件的路径
 */
export declare function getPackageJsonInfo(filePath: string): PackageInfoType | undefined;
/**
 * 根据node的规则获取包的入口文件
 *
 * @param packageJsonObject oh-package.json / package.json的对象
 */
export declare function findNodeEntryFiles(packageJsonObject: any): string[];
export {};
