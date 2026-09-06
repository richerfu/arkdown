# arkdown-ets-checker 设计

`arkdown-ets-checker` 是构建进程内的 OXC TypeScript/ArkTS 项目 checker。
它不调用 SDK Node checker，也不提供独立 CLI。公开入口接受显式的
`CheckOptions`，由 build/check 服务传入 root files、tsconfig、package alias、
module paths 和 SDK declarations。

## 设计前提

checker 强制使用 isolated-declarations surface，将跨文件环境和单文件检查
状态分离：

1. 每个文件的导出 surface 可以独立抽取。
2. surface 在 link 后形成只读、`Send + Sync` 的 `ProgramEnv`。
3. 每个检查任务只保留自己的 AST 和局部类型 scratch，结束后即可释放。

OXC AST 包含 `Cell`，不能直接作为线程共享环境。`ir.rs` 的 owned IR 是并行
安全和内存边界，不是可选缓存。

## Pipeline

```text
load（按发现 wave 并行）
  parse → isolated declarations → surface → oxc_resolver
  → 新解析到的 .d.ts/.d.ets 加入下一 wave

link（确定性单线程归并）
  SymbolId / export tables / re-export chains / frozen TypeTable

check（逐文件并行）
  reparse → local annotations and inference → tri-state relation
  → diagnostics sorted by source position
```

`CheckOptions` 是唯一输入事实。resolver 使用项目 alias、额外 package roots、
tsconfig paths 和 ArkTS/TypeScript 扩展名；checker 自身不重新发现工程模型或
SDK。

## 类型和关系

`TypeTable` 使用稳定的 `TypeId`。全局类型表只读共享，每个文件的
`TypeSink`/local types 追加在全局 ID 空间之后。

assignability 返回 `True`、`False` 或 `Unknown`：

- `True`：确定可赋值。
- `False`：确定不可赋值，允许产生诊断。
- `Unknown`：当前模型无法证明，不产生诊断。

该规则避免对未建模语义产生猜测性错误。不能为了提高“覆盖数”而把
`Unknown` 当作 `False`，也不能恢复 SDK checker 作为隐藏 fallback。

当前 IR/关系覆盖 primitive/literal、union、array/tuple/readonly、object、
function shape、class/interface/type alias、enum、property/call/construct 的已实现
子集，以及带约束的部分 generic 实例化。未完整建模的 conditional、mapped、
indexed、`typeof`、template/import/infer/constructor/this type、intersection
关系、复杂 namespace、declaration merging、global augmentation 和完整
control-flow narrowing 会保持 opaque/unsupported。

## 当前诊断

除 OXC parse 和 isolated-declarations 诊断外，当前源码明确构造下列
TypeScript-compatible code：

- 模块与导出：TS1192、TS2305、TS2307、TS2613、TS2694。
- 赋值与调用：TS1360、TS2322、TS2345、TS2352、TS2353、TS2739、
  TS2740、TS2741、TS4104。
- 类型参数和成员：TS2314、TS2339、TS2344、TS2416、TS2420。
- 表达式和控制流：TS2355、TS2362、TS2363、TS2366。

新增诊断必须由 checker 已能证明的 typed fact 驱动，并用真实 TypeScript 或
ets2bundle 源码契约保护。诊断顺序必须保持确定性；`skipLibCheck` 只清除声明
文件自身的诊断。

## 后续工作

- 按 ets2bundle 实际调用和语料补足复杂类型、声明合并、namespace/global
  augmentation 与 control-flow semantics。
- 以 surface/content hash 做文件级增量检查，并保持失效闭包确定性。
- 保持 checker 为嵌入式 Rust domain；任何新 CLI 行为都只能从 JS CLI 进入
  typed N-API 服务。
