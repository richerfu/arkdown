# crates/arkdown-ohpm

Action: Add/Keep.

## 目标职责

`arkdown-ohpm` 拥有 OHPM 包获取机制：`oh-package-lock.json5` 解析、全局 cache 布局、offline/prefer-offline 查找、registry metadata fetch、tarball integrity 校验、安全解压、cache install。它不构建 dependency graph，不处理 profile，不判断 HAR/HSP 业务语义。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| plugin factory 中 `ohpm i --all --target_path` | `src/plugin/factory/plugin-factory.js` | `acquisition.rs` |
| oh-package loader/cache | `src/utils/loader/file/oh-package-loader.js` | `lockfile.rs`, `package_name.rs` |
| dependency manager 读取 remote package root | `src/project/dependency/dependency-manager.js` | `cache.rs` output |
| registry/proxy 参数 | `src/sdk/sdk-util.js::proxyFun`, npm utils | `registry.rs` |
| tarball 下载和校验 | OHPM install 语义 | `tarball.rs`, `integrity.rs` |

## 目标目录

```text
src/
  lib.rs
  error.rs
  package_name.rs
  lockfile.rs
  cache.rs
  registry.rs
  tarball.rs
  integrity.rs
  acquisition.rs
```

## 文件作用和内部结构

- `package_name.rs`: scoped package name parser、version/range raw value，禁止用路径字符串推导包名。
- `lockfile.rs`: lockfile schema parse，输出 `LockfilePackageRef` 和 content fingerprint。
- `cache.rs`: global/local OHPM cache layout、cache entry ID、atomic install marker。
- `registry.rs`: registry metadata client、proxy/no-proxy 配置、offline/prefer-offline 策略输入。
- `tarball.rs`: `.tgz` 下载结果、safe extraction、path traversal 防护、deterministic file order。
- `integrity.rs`: sha512/integrity 校验。
- `acquisition.rs`: `PackageAcquisitionPlan` 执行器，输入 package request，输出 acquired package root。
- `error.rs`: typed OHPM errors，包含 package name/version/cache path/registry URL diagnostic。

## 串联关系

```text
arkdown-session::PackageAcquisitionPlanner
  -> arkdown-ohpm::PackageAcquirer
  -> PackageSnapshot root
  -> arkdown-session::DependencyGraphBuilder
```

`arkdown-ohpm` 只回答“包在哪里、是否已获取、完整性是否正确”。它不回答“这个包是 HAR、HSP、ByteCode HAR 还是 runtimeOnly”。

## 迁移要点

- 从 `arkdown-profile` 移出 registry/cache/lockfile/tarball 逻辑。
- offline 模式不得发 HTTP。
- prefer-offline 必须先命中 cache，再按 request fallback registry。
- 同一 package/version 的 cache install 需要按 cache entry ID 串行化。

