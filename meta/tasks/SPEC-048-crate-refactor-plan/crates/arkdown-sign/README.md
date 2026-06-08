# crates/arkdown-sign

Action: Keep.

## 目标职责

`arkdown-sign` 负责 HAP/HSP/HAR 签名和签名块处理。它消费 `SignPlan`，只使用 plan 中的 keystore/profile/cert/input/output/tool path，不发现项目、不读取 build profile。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| SignHap | `src/tasks/sign-hap.js` | `signer.rs` |
| SignHsp | `src/tasks/hsp/sign-hsp.js` | `signer.rs` |
| SignModel and command builders | `src/tasks/sign/command-builder-impl/*.js` | `plan.rs`, `tool.rs` |
| keystore/password decrypt | `common-sign-command-builder.js`, `decipher-util.js` | `keystore.rs` |
| signing block | signing implementation | `signing_block.rs`, `pkcs7.rs`, `digest.rs`, `zip.rs` |
| validate sign | `src/tasks/validate-sign.js` | plan validation input |

## 目标目录

```text
src/
  lib.rs
  error.rs
  plan.rs
  signer.rs
  tool.rs
  keystore.rs
  digest.rs
  pkcs7.rs
  signing_block.rs
  zip.rs
```

## 文件作用和内部结构

- `plan.rs`: `SignPlan`，sign type、input archive、output archive、material paths、profile/cert/keystore metadata。
- `signer.rs`: high-level sign executor。
- `tool.rs`: external sign tool invocation or Rust signer bridge。
- `keystore.rs`: keystore loading/decrypt material handling。
- `digest.rs`, `pkcs7.rs`, `signing_block.rs`, `zip.rs`: low-level signing implementation。
- `error.rs`: signing errors with material path attachment。

## 串联关系

```text
arkdown-pack unsigned artifact
  -> SignPlan
  -> arkdown-sign
  -> signed artifact
```

## 迁移要点

- signing config normalization 在 `arkdown-config/session`，sign crate 只执行。
- 签名最终写入必须串行。
- 不依赖 `arkdown-session`。

