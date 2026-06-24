/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/data/parameters.d.ts
 */
import { CoreProperties, ParamValue } from '../internal/data/global-core-parameters';
export interface Parameter {
    getProperty(key: string): any | undefined;
    setProperty(key: string, value: any): void;
    getProperties(): Properties;
    getExtParam(key: string): string | undefined;
    getExtParams(): Record<string, string>;
    getStartParams(): StartParam;
    getWorkspaceDir(): string;
}
export interface ExternalStartParam {
    analyze: string | boolean;
    daemon: boolean;
    parallel: boolean;
    incremental: boolean;
    logLevel: string;
    typeCheck: boolean;
    optimizationStrategy: string;
    hotCompile?: boolean;
    hotReloadBuild?: boolean;
}
export type Properties = Readonly<CoreProperties>;
export type StartParam = Readonly<ExternalStartParam>;
export declare class ExternalParameter implements Parameter {
    private logger;
    /**
     * hvigor-config-schema.json 中的 properties 字段 schema，只在 setProperty 中用来校验
     * 在第一次调用 setProperty 时才会赋值
     * @private
     */
    private propertiesSchema?;
    getExtParam(key: string): string | undefined;
    getExtParams(): Readonly<Record<string, string>>;
    private _getExtParams;
    getProperties(): Readonly<Record<string, ParamValue>>;
    private _getProperties;
    getProperty(key: string): ParamValue;
    private _getProperty;
    setProperty(key: string, value: any): void;
    private _setProperty;
    private getCallerPath;
    getStartParams(): StartParam;
    private _getStartParams;
    getWorkspaceDir(): string;
    private _getWorkspaceDir;
}
