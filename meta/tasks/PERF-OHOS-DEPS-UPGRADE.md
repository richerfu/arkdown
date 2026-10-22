# ohos 依赖升级与 Hvigor 构建性能对照

2026-10-22。范围：`chore/upgrade-ohos-deps` 分支把 ohos-rs 的 oxc、rolldown、
tsrs 升级到各自 `ohos` 分支最新提交后，在一个生产 HarmonyOS 工程上的 clean
debug HAP 构建对照。

两组结论：

1. 相对升级前的 Arkdown，clean 构建中位墙钟下降 **15.2%**，收益集中在
   Bundling（−14.9%）；峰值 RSS 基本不变。
2. 相对同一工程上的 Hvigor 6.26.4，升级后的 Arkdown cold build 少 **86.6%**
   墙钟、少 **75.3%** 峰值物理 footprint、少 **41.1%** 峰值树 RSS；
   unchanged-source 热构建慢 **2.90 倍**，但 RSS/footprint 更低。

本轮只描述该机器、该工程和该命令下的观测值，不建立跨机器性能合同。

## 依赖版本

| 组件 | 升级前 | 升级后 |
| --- | --- | --- |
| oxc (`ohos-rs/oxc`) | `3f2b4b7d5338c10014c973e0db874ec2d3b655db` | `6d42deb0af488a59c3d2ba06e01fc60d87b86355` |
| rolldown (`ohos-rs/rolldown`) | `63112ceaa35eb24750a09d0ce3ae6f2f8074ed42` | `4f27b5fe103e661090848aaaa4f7d3084bc2e49c` |
| tsrs (`ohos-rs/tsrs`) | `4d7e69df3847b5e0e5708b22a3f713082e5e6d02` | `a59ee9bdc7d632b6d797711291375e8511674f82` |
| `oxc_sourcemap` | `8.1.2` | `9.0.0` |
| `oxc_resolver` | `11.21.3` | `11.24.3` |

升级伴随 oxc 0.155 的 `JSStr`/`StaticPropertyName` 与 rolldown 1.2.14 的
`Pluginable`/`MatchGroup` API 迁移；迁移本身以工作区检查和测试保证语义，不把
API 迁移的收益与上游性能变化分开归因。

## 环境与输入

- Apple M2 Pro（12 核），16 GiB 内存，macOS 15.6.1（24G90）。
- Node.js `v22.23.2`，rustc `1.99.0`；Arkdown 使用 release-mode native addon
  （LTO、`codegen-units=1`），`NODE_OPTIONS=--max-old-space-size=8192`。
- DevEco Studio `26.0.0.821` SDK；Hvigor `6.26.4`，`DEVECO_SDK_HOME` 指向该 SDK。
- 输入为一个生产 HarmonyOS 应用：11,627 个 `.ts`、23 个 `.ets`，已物化
  `oh_modules` 约 4.8 GiB。
- 构建命令：

  ```sh
  # Hvigor
  hvigorw --mode module -p product=default -p buildMode=debug \
    assembleHap --no-daemon

  # Arkdown
  arkdown clean --project .
  arkdown build --project . --target hap --mode debug --clearScreen false
  ```

  cold build 前执行各自公开的 clean；hot build 不 clean，输入保持不变。
  依赖、SDK 文件和 OS page cache 保持物化。

## 测量方法

- 使用进程树采样脚本：每 100 ms 累加树内全部进程的 RSS，每 1 s 调用
  `footprint -f bytes` 累加 `phys_footprint`。采样阻塞时实际间隔会略大于标称值。
- Hvigor 1 组 cold/hot；Arkdown full checker 与 `--skip-checker` 各 2 组
  cold/hot，表中取中位数。Hvigor wrapper 自身使用
  `--max-old-space-size=8192 --max-semi-space-size=16 --expose-gc`。
- 本轮 cold 时间受 I/O 影响明显：Arkdown full checker 两次为 78.72 s 和 89.43 s。
  因此不对 `--skip-checker` 与 full checker 的 cold 差值做因果结论。

## Hvigor 与 Arkdown 对照

| 指标 | Hvigor 6.26.4 | Arkdown full checker | Arkdown `--skip-checker` |
| --- | ---: | ---: | ---: |
| Cold wall | 629.69 s | 84.08 s | 98.02 s |
| Hot wall | 5.96 s | 17.28 s | 15.84 s |
| Cold peak tree RSS | 2.90 GiB | 1.71 GiB | 1.68 GiB |
| Hot peak tree RSS | 0.54 GiB | 0.42 GiB | 0.43 GiB |
| Cold peak tree footprint | 9.68 GiB | 2.39 GiB | 2.37 GiB |
| Hot peak tree footprint | 0.49 GiB | 0.34 GiB | 0.34 GiB |

相对 Hvigor，Arkdown full checker：

- cold wall −86.6%（7.49 倍），cold RSS −41.1%，cold footprint −75.3%；
- hot wall 2.90 倍（17.28 s vs 5.96 s），hot RSS −21.9%，hot footprint −30.2%。

Hvigor cold 中 `CompileArkTS` 为 523.7 s，`PackageHap` 55.3 s，`SignHap`
37.6 s，合计构成 10 分 25 秒的构建。Arkdown cold 中位阶段为：Preparing 6.4 s、
Profiles/resources 2.4 s、Bundling 36.6 s、ABC 15.8 s、Packaging 22.3 s。
Arkdown hot 阶段为 Preparing 6.9 s、Profiles/resources 3.3 s、Bundling 0.5 s、
ABC 0 s、Packaging 7.4 s。

原始采样：

| 运行 | wall | 树 RSS | 树 footprint |
| --- | ---: | ---: | ---: |
| Hvigor cold | 629.69 s | 2.90 GiB | 9.68 GiB |
| Hvigor hot | 5.96 s | 0.54 GiB | 0.49 GiB |
| Arkdown full cold #1/#2 | 78.72 / 89.43 s | 1.74 / 1.68 GiB | 2.39 / 2.40 GiB |
| Arkdown full hot #1/#2 | 16.21 / 18.34 s | 0.42 / 0.43 GiB | 0.34 / 0.34 GiB |
| Arkdown skip cold #1/#2 | 101.04 / 95.00 s | 1.76 / 1.59 GiB | 2.38 / 2.37 GiB |
| Arkdown skip hot #1/#2 | 16.35 / 15.33 s | 0.45 / 0.41 GiB | 0.33 / 0.34 GiB |

## 升级前后对照

升级前后各用对应 release addon 交错执行三轮 clean 构建，使用
`/usr/bin/time -l` 记录 Node 进程及其已回收子进程的最大 RSS（不是进程树采样，
绝对值不能与上表直接比较）：

| 指标（中位数） | 升级前 | 升级后 | 变化 |
| --- | ---: | ---: | ---: |
| 墙钟 | 105.72 s | 89.61 s | **−15.2%** |
| user CPU | 108.99 s | 105.49 s | −3.2% |
| sys CPU | 65.64 s | 66.91 s | +1.9% |
| 进程最大 RSS | 1.646 GiB | 1.673 GiB | +1.6% |

分阶段中位数：

| 阶段 | 升级前 | 升级后 | 变化 |
| --- | ---: | ---: | ---: |
| Preparing project | 9.87 s | 9.47 s | −4.1% |
| Profiles and resources | 3.44 s | 2.77 s | −19.5% |
| Bundling | 40.93 s | 34.85 s | **−14.9%** |
| Compiling ABC | 18.10 s | 16.83 s | −7.0% |
| Packaging | 35.60 s | 28.54 s | −19.8% |

三轮原始值：

| 轮次 | 升级前墙钟 / RSS | 升级后墙钟 / RSS |
| --- | ---: | ---: |
| 1 | 91.49 s / 1.740 GiB | 89.61 s / 1.673 GiB |
| 2 | 118.82 s / 1.615 GiB | 96.17 s / 1.645 GiB |
| 3 | 105.72 s / 1.646 GiB | 81.57 s / 1.713 GiB |

Bundling 是唯一在三个轮次中方向一致且幅度稳定的收益来源。ABC 与 Packaging
的单轮值分别跨 12.36–19.21 s、20.23–43.32 s，不能据此声明这两个阶段有确定优化。
墙钟下降幅度大于 user CPU 下降幅度，说明部分收益来自等待、并行或 I/O 行为变化；
本轮没有做 profile，不能把收益归因到某个具体 upstream commit。

## 内存结论

升级前后进程最大 RSS 中位数从 1.646 GiB 变为 1.673 GiB（+1.6%），区间重叠，
不能声明依赖升级降低了峰值内存。相对 Hvigor，Arkdown cold/hot 的进程树 RSS
与 footprint 都明显更低，但这是实现差异，不是本次升级带来的增量。

## 验证

- `cargo check --workspace --all-targets`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- 受影响 crate（`arkdown-ets-transform`、`arkdown-obfuscate`、
  `arkdown-ets-checker`、`arkdown --lib`）：684 项测试通过，0 失败。
- 所有 cold/hot 构建均成功，debug HAP 产物与签名正常。

## 决策

保留依赖升级。构建时间收益明确集中在 Bundling；内存没有可测的升级收益，不据此
调整 allocator、并发或缓存策略。热构建仍比 Hvigor 慢约 2.9 倍，后续若继续优化，
应优先归因 Preparing/packaging/signing，而不是 checker。
