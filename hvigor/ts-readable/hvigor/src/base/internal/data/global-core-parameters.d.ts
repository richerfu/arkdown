/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/data/global-core-parameters.d.ts
 */
import { Level } from 'log4js';
export declare class CoreParameter {
    private _properties;
    private _extParams;
    private _startParams;
    private _workspaceDir;
    get properties(): CoreProperties;
    set properties(value: CoreProperties);
    get extParams(): Record<string, string>;
    set extParams(value: Record<string, string>);
    get startParams(): CoreStartParam;
    get workspaceDir(): string;
    set workspaceDir(value: string);
    clean(): void;
}
export declare enum AnalyzeMode {
    NORMAL = 0,
    ADVANCED = 1,
    FALSE = 2,
    TRACE = 3
}
export declare enum OptimizationStrategy {
    PERFORMANCE = "performance",
    MEMORY = "memory"
}
export declare const AnalyzeModeMap: Map<string | boolean, AnalyzeMode>;
export declare const OldAnalyzeModeMap: Map<string | boolean, string>;
export declare const AnalyzeModeKeyMap: Map<number, string | boolean>;
export declare const LogLevelMap: Map<string, Level>;
export declare const defaultStartParam: CoreStartParam;
export declare const defaultProperties: CoreProperties;
export interface CoreStartParam {
    hvigorfileTypeCheck: boolean;
    parallelExecution: boolean;
    incrementalExecution: boolean;
    printStackTrace: boolean;
    daemon: boolean;
    analyze: AnalyzeMode;
    logLevel: Level;
    optimizationStrategy: string;
    hotCompile?: boolean;
    hotReloadBuild?: boolean;
    customizedParam?: any;
}
export interface CoreProperties {
    ohosArkCompileMaxSize?: number;
    hvigorPoolMaxSize?: number;
    hvigorPoolMaxCoreSize?: number;
    hvigorPoolCacheCapacity?: number;
    hvigorPoolCacheTtl?: number;
    enableSignTask?: boolean;
    skipNativeIncremental?: boolean;
    'hvigor.analyzeHtml'?: boolean;
    'ohos.compile.lib.entryfile'?: boolean;
    'ohos.sign.har'?: boolean;
    [key: string]: ParamValue;
    'ohos.arkCompile.sourceMapDir'?: string;
    hvigorMemoryThreshold?: number;
    'ohos.processLib.optimization'?: boolean;
    'ohos.obfuscationRules.optimization'?: boolean;
    'ohos.byteCodeHar.integratedOptimization'?: boolean;
    'ohos.integratedHsp.useGetCurrentBundleName'?: boolean;
    enableOverridesDependencyMap?: boolean;
}
export type ParamValue = string | boolean | number | undefined;
export declare const coreParameter: CoreParameter;
