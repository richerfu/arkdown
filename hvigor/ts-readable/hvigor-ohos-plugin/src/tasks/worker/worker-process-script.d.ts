/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/worker-process-script.d.ts
 */
import { LogCombineType } from '../../utils/log/log-combine-type';
type CommandInput = {
    moduleName?: string;
    taskName?: string;
    commandLine: string[];
    integratedHspCommand: string[] | undefined;
    commandOptions?: object;
    logCompileType?: LogCombineType;
    logLevel?: string;
};
export declare function warmUp(): Promise<void>;
export declare function generalExecute(workData: CommandInput): Promise<void>;
export declare function packageHar(workData: CommandInput): Promise<string>;
export {};
