/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/project-config/mock-info.d.ts
 */
export interface MockInfo {
    /**
     * mock相关参数
     */
    mockParams?: {
        /**
         * mock 装饰器名称
         */
        decorator: string;
        /**
         * mock 三方包完整包名
         */
        packageName: string;
        /**
         * src/main/ets
         */
        etsSourceRootPath: string;
        /**
         * mock 配置文件的路径,文件不存在时赋值 undefined
         */
        mockConfigPath: string | undefined;
        /**
         * mock-config中key -> 模块信息，目前只存在har和hsp
         */
        mockConfigKey2ModuleInfo: Record<string, MockConfigModuleInfo>;
        /**
         * mock 配置文件是否有变化
         */
        isMockConfigChanged?: boolean;
        source2ModuleIdMap: Map<string, string>;
    };
}
export interface MockConfigModuleInfo {
    /**
     * key对应的路径
     */
    filePath: string;
    meta: {
        /**
         * 模块的根路径
         */
        pkgPath: string;
        /**
         * 模块名称
         */
        pkgName: string;
    };
}
