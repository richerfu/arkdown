/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/command-builder-modifier.d.ts
 */
import { BaseCommandBuilder } from './base-command-builder';
export interface CommandModifier {
    modify: (command: string[]) => string[];
}
export declare enum CommandBuilderType {
    NONE = 0,
    CMAKE = 1,
    NINJA = 2
}
export declare function registryCommandModifier(cmdBuilderType: CommandBuilderType, modifier: CommandModifier): void;
export declare class CommandBuilderModifier<T extends BaseCommandBuilder> {
    static modifiers: Map<CommandBuilderType, CommandModifier>;
    private readonly commandBuilder;
    private readonly cmdBuilderType;
    constructor(commandBuilder: T, cmdBuilderType: CommandBuilderType);
    static registerModifier(cs: CommandBuilderType, mod: CommandModifier): void;
    wrapCommandBuilder(): T;
}
