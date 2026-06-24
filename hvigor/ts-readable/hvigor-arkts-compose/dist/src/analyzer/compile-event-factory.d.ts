/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/analyzer/compile-event-factory.d.ts
 */
import { CompileEvent } from './compile-event';
/**
 * 对应hvigor中的MetricFactory
 *
 * @since 2023/6/2
 */
export declare class CompileEventFactory {
    static compileEvents: Map<string, CompileEvent>;
    static hookEventFactories: Map<string, HookEventFactory>;
    static createEvent(name: string, totalTime: number, frequency: number, tid?: string): CompileEvent;
    static removeIf(predicate: (compileEvent: CompileEvent) => boolean): void;
    static getEvents(): Map<string, CompileEvent>;
    static removeEvent(eventId: string): void;
    static getEventById(id: string): CompileEvent | undefined;
    static getHookEventFactory(pluginName: string, hookName: string): HookEventFactory;
    static clear(): void;
}
/**
 * 提供给rollup插件内部进行打点的事件工厂
 */
export declare class HookEventFactory {
    private readonly category;
    constructor(category: string);
    createEvent(name: string): CompileEvent | undefined;
}
