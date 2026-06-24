/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/store/tcb-store.d.ts
 */
import { TCB } from '../model/tcb';
/**
 * TCB存储
 * 维护TCB数据
 *
 * @since 2022/8/16
 */
export declare class TcbStore {
    private static tcbMap;
    static getTCB(id: string): TCB | undefined;
    static clear(): void;
    static add(tcb: TCB): void;
}
