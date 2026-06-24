/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/configure/startup-options.d.ts
 */
export interface AppPreloadHintStartupTasksObj {
    name: string;
    srcEntry: string;
    dependencies: string[];
    excludeFromAutoStart: boolean;
    runOnThread: string;
    ohmurl?: string;
    moduleName?: string;
    startupConfigPath?: string;
    isRemoteByteCodeHar?: boolean;
}
export interface StartupTaskObj {
    name: string;
    srcEntry: string;
    srcEntryAbsolutePath?: string;
    dependencies: string[];
    excludeFromAutoStart: boolean;
    runOnThread: string;
    waitOnMainThread: boolean;
    ohmurl?: string;
    moduleName?: string;
    obfuscationSrcEntry?: string;
    startupConfigPath?: string;
}
interface SystemPreloadHintStartupTask {
    name: string;
    srcEntry: string;
    ohmurl: string;
}
export interface StartupOptions {
    appPreloadHintStartupTasks?: AppPreloadHintStartupTasksObj[];
    startupTasks?: StartupTaskObj[];
    configEntry?: string;
    systemPreloadHintStartupTasks?: SystemPreloadHintStartupTask[];
}
export {};
