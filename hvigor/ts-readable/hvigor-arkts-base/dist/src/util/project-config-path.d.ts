/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/util/project-config-path.d.ts
 */
import { ModuleJSON, SerializablePluginCache } from 'rollup';
/**
 * 将project-path和sdk-path放到本类中方便调用
 */
declare class ProjectConfigPath {
    projectTopPath: string | undefined;
    sdkPath: string | undefined;
    private sdkUnderlinePath;
    private sdkNativePath;
    private sdkNativeUnderlinePath;
    private buildDir;
    private buildCacheProjectDir;
    init(projectTopDir: string, sdkPath: string, buildDir: string): void;
    /**
     * 将rollup的pluginsCache中的所有路径替换掉
     * @param plugins
     * @param mode 如果为true 则将所有路径换为占位符，如果为false，则将所有占位符换为路径
     */
    replacePluginsCachePath(plugins: Record<string, SerializablePluginCache>, mode: boolean): void;
    /**
     * 将rollup的moduleCache中的所有路径替换掉
     * @param module
     * @param mode 如果为true 则将所有路径换为占位符，如果为false，则将所有占位符换为路径
     */
    replaceModuleCachePath(module: ModuleJSON, mode: boolean): void;
    private handlePluginsCache;
    private handleModuleCache;
    private replaceExportAllDeclarationCache;
    private replaceImportCache;
    private replaceStringProperties;
    private replaceToNativePath;
    private replaceToPlaceholder;
    private getBuildCacheProjectDir;
}
export declare const projectConfigPath: ProjectConfigPath;
export {};
