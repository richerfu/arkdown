/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/npm-command-builder.d.ts
 */
import { BaseCommandBuilder } from './base-command-builder';
export declare class NpmCommandBuilder extends BaseCommandBuilder {
    constructor(npmPath?: string);
    runScript(script: string): NpmCommandBuilder;
    addScriptsOptions(): NpmCommandBuilder;
    install(): NpmCommandBuilder;
}
