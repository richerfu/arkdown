/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/server/daemon-server-listener-registry.d.ts
 */
import { HvigorDaemonServer } from './hvigor-daemon-server';
/**
 * 在daemon process中的server上需要添加的事件监听
 */
export declare class DaemonServerListenerRegistry {
    registryAllListener(hvigorDaemonServer: HvigorDaemonServer): void;
    /**
     * 当server建立起一个新的socket连接时触发，完成向会话中心注册socket等工作
     * @private
     */
    private onConnection;
    /**
     * 当server侧主动断开连接时手动触发，完成会话清理等工作
     * @private
     */
    private onCloseConnection;
    private onCloseServer;
    /**
     * server发生错误时执行的回调
     */
    private onError;
    /**
     * server开始监听端口后执行的回调方法
     * @private
     */
    private onListening;
    /**
     * server监听端口失败时的回调
     * @private
     */
    private onListenFailed;
}
