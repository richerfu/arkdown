/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/options/hvigor-cli-options.d.ts
 */
export interface HvigorCliOptions {
    _: string[];
    help?: boolean;
    cwd?: string;
    require?: string;
    prop?: string | string[];
    mode?: string;
    sync?: boolean;
    error?: boolean;
    warn?: boolean;
    info?: boolean;
    debug?: boolean;
    parallel?: boolean;
    incremental?: boolean;
    stacktrace?: boolean;
    enableBuildScriptTypeCheck?: boolean;
    daemon?: boolean;
    watch?: boolean;
    stopDaemon?: boolean;
    stopDaemonAll?: boolean;
    startDaemon?: boolean;
    statusDaemon?: boolean;
    nodeHome?: string;
    typeCheck?: boolean;
    pnpmFrozenLockfile?: boolean;
    analyze?: boolean | string;
    verboseAnalyze?: boolean;
    completeCommand?: string;
    config?: string[];
    maxOldSpaceSize?: number;
    Xmx?: number;
    env?: {
        [key: string]: string | undefined;
    };
    hotCompile?: boolean;
    optimizationStrategy?: string;
    hotReloadBuild?: boolean;
    experimentOptions?: {
        [key: string]: string | undefined;
    };
}
