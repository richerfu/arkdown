/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/hook/hvigor-lifecycle-hook.js
 */
import * as hook_const_js_1 from "../../../../common/const/hook-const";
import * as hvigor_core_js_1 from "../../../external/core/hvigor-core";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("HvigorLifecycleHook");
export class HvigorLifecycleHook {
  constructor() {
    this.configEvaluatedFnList = [], this.nodesInitializedFnList = [], this.beforeNodeEvaluateFnList = [], this.afterNodeEvaluateFnList = [], this.nodesEvaluatedFnList = [], this.taskGraphResolvedFnList = [], this.buildFinishedFnList = [], this.onWatchWorkerMessageFnList = [], this.strategies = {
      [hook_const_js_1.HookType.configEvaluated]: async s => {
        await this.runFn(this.configEvaluatedFnList, s);
      },
      [hook_const_js_1.HookType.nodesInitialized]: async s => {
        await this.runFn(this.nodesInitializedFnList, s);
      },
      [hook_const_js_1.HookType.beforeNodeEvaluate]: async s => {
        await this.runFn(this.beforeNodeEvaluateFnList, s);
      },
      [hook_const_js_1.HookType.afterNodeEvaluate]: async s => {
        await this.runFn(this.afterNodeEvaluateFnList, s);
      },
      [hook_const_js_1.HookType.nodesEvaluated]: async s => {
        await this.runFn(this.nodesEvaluatedFnList, s);
      },
      [hook_const_js_1.HookType.taskGraphResolved]: async s => {
        await this.runFn(this.taskGraphResolvedFnList, s);
      },
      [hook_const_js_1.HookType.buildFinished]: async s => {
        await this.runFn(this.buildFinishedFnList, s);
      },
      [hook_const_js_1.HookType.onWatchWorkerMessage]: async s => {
        await this.runFn(this.onWatchWorkerMessageFnList, s);
      }
    };
  }
  static getInstance() {
    return HvigorLifecycleHook.instance || (HvigorLifecycleHook.instance = new HvigorLifecycleHook()), HvigorLifecycleHook.instance;
  }
  async runFn(s, o) {
    for (const t of s) t && (await t.bind(this)(o));
  }
  async runHook(s, o, t) {
    const e = null == t ? undefined : t.createSubEvent(`"${hook_const_js_1.HookType[s]}" hook function`, "").start();
    try {
      await this.strategies[s].bind(this)(o), null == e || e.stop().setLog();
    } catch (o) {
      if (o instanceof Error) {
        if (hvigor_logger_1.ErrorUtil.isAdaptorError(o)) throw o;
        null == e || e.stop().setLog(), _log.printErrorExit("FAILED_TO_EXECUTE_HOOK", [s, o.message], [[s]], o.stack);
      }
    }
  }
  configEvaluated(s) {
    this.configEvaluatedFnList.push(s);
  }
  nodesInitialized(s) {
    this.nodesInitializedFnList.push(s);
  }
  beforeNodeEvaluate(s) {
    this.beforeNodeEvaluateFnList.push(s);
  }
  afterNodeEvaluate(s) {
    this.afterNodeEvaluateFnList.push(s);
  }
  nodesEvaluated(s) {
    this.nodesEvaluatedFnList.push(s);
  }
  taskGraphResolved(s) {
    this.taskGraphResolvedFnList.push(s);
  }
  buildFinished(s) {
    this.buildFinishedFnList.push(s);
  }
  onWatchWorkerMessage(s) {
    this.onWatchWorkerMessageFnList.push(s);
  }
  clear() {
    this.configEvaluatedFnList = [], this.nodesInitializedFnList = [], this.beforeNodeEvaluateFnList = [], this.afterNodeEvaluateFnList = [], this.nodesEvaluatedFnList = [], this.taskGraphResolvedFnList = [], this.buildFinishedFnList = [], this.onWatchWorkerMessageFnList = [], hvigor_core_js_1.hvigorCore.clearHvigorInternalHookList();
  }
}
