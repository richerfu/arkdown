/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/configure/app-startup-options.d.ts
 */
import { RequiredNamed } from '../options';
export declare namespace AppStartupOptions {
    interface AppStartupOpt extends RequiredNamed {
        startupTasks: StartupTaskObj[];
        configEntry: string;
    }
    interface StartupTaskObj extends RequiredNamed {
        name: string;
        srcEntry: string;
        dependencies: string[];
        excludeFromAutoStart: boolean;
        runOnThread: string;
        waitOnMainThread: boolean;
    }
}
