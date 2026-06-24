/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/project-config/build-environment.d.ts
 */
export interface BuildEnvironment {
    /**
     * 当前工程的runtimeOS
     */
    runtimeOS: string;
    /**
     * 当前工程的sdkInfo
     */
    sdkInfo: string;
    /**
     * 当前工程使用的api
     */
    apiVersion: number;
    /**
     * 编译工程使用的 sdk中的 ets-loader 根路径
     */
    etsLoaderPath: string;
    /**
     * compileSdkVersion
     */
    compileSdkVersion?: number;
    /**
     * compatibleSdkVersion
     */
    compatibleSdkVersion?: number;
    /**
     * originCompatibleSdkVersion
     */
    originCompatibleSdkVersion?: number | string | undefined;
    /**
     * compatibleSdkVersionStage
     */
    compatibleSdkVersionStage?: string;
    /**
     * ets-loader的版本号
     */
    etsLoaderVersion: string;
    /**
     * etsLoader的ReleaseType
     */
    etsLoaderReleaseType?: string;
    /**
     * 当前编译环境API版本等的标志
     */
    compileEnv?: string;
    /**
     * 本次构建的Build ID
     */
    buildId?: string;
}
