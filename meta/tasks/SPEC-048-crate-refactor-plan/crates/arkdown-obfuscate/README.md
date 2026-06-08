# crates/arkdown-obfuscate

Action: Keep.

## 目标职责

`arkdown-obfuscate` 负责 release obfuscation/minify/rename/rules 处理。它消费 `ObfuscationPlan`，规则文件 inventory 和 SDK capability 来自 session/build plan。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| ArkCompile obfuscation options | `ark-compile.js::getObfuscationOptions`, `computeObfuscationFileHash` | `plan.rs`, `rules.rs` |
| SDK generate consumer ob config | `load-sdk-functions.js::generateObfuscationFile` | `consumer.rs` |
| HAR ProcessObfuscationFiles | `src/tasks/har/process-obfuscation-files.js` | `consumer.rs`, `rules.rs` |
| terser/minify | `load-sdk-functions.js::terserPlugin` | `minifier.rs` |

## 目标目录

```text
src/
  lib.rs
  error.rs
  plan.rs
  rules.rs
  transform.rs
  rename.rs
  minifier.rs
  consumer.rs
```

## 文件作用和内部结构

- `plan.rs`: `ObfuscationPlan`，enable、rule files、consumer files、dependency rule metadata、output dirs。
- `rules.rs`: rule parse/merge/fingerprint。
- `transform.rs`: source transform entry。
- `rename.rs`: symbol rename plan。
- `minifier.rs`: minify facade。
- `consumer.rs`: consumer ob config generation。
- `error.rs`: obfuscation diagnostics。

## 串联关系

```text
BuildPlanBuilder
  -> ObfuscationPlan
  -> arkdown-obfuscate
  -> bundle/ABC/package inputs
```

## 迁移要点

- 移除 rule discovery 的 ad hoc traversal。
- obfuscation hash 纳入 `InputFingerprintSet`。
- debug build 下的禁用逻辑由 plan 决定，crate 只执行。

