# UIM × AntiWebUI

用同一套 `el!` 语法构建 [AntiWebUI](https://github.com/bea4dev/AntiWebUI) 原生任务面板，
包含任务完成切换、优先级筛选、响应式统计、空状态和重置。

从 gds-uim 仓库根目录运行：

```sh
cargo run --locked --manifest-path examples/anti_webui/Cargo.toml
```

Linux 需要图形会话、可用的图形驱动，以及 Fontconfig/FreeType、Wayland、XKB/XCB
开发库和 pkg-config。依赖启用了上游的
Wayland、X11、macOS 和 Windows 后端；Cargo 按目标平台选择对应实现。

也可以通过 AntiWebUI 的离屏渲染生成 PNG，不需要打开窗口，但仍需要可用的 GPU
或软件 Vulkan 驱动：

```sh
cargo run --locked --manifest-path examples/anti_webui/Cargo.toml -- examples/anti_webui/preview.png
```

## UIM 与响应式更新

`el!` 负责构建器调用，响应式依赖由 AntiWebUI 的 `signal`、动态文本和 `dyn_view`
管理。普通 UIM `if` 在构建时求值；需要随信号更新的条件放进响应式回调：

```rust,ignore
el! {
    .div.{ flex; flex_col; gap: 12.0 }.[
        (move () => board.summary).text,
        (move () => board.task_list).dyn_view,
    ]
}
```

示例使用 `.div`、`.{ ... }`、`.[ ... ]`、`..items`、嵌套分组、`^(...)` 值分支、
`move` 闭包以及 `task->priority` 字段访问。事件处理和数据修改保留在普通 Rust
函数中。样式值使用 AntiWebUI 的逻辑像素 `f32`，颜色使用它的 `rgb` 函数。

UIM 不提供框架方法适配：`#` 会调用 `id`，`#?` 会调用 `debug_selector`，
使用前需要对应 builder 确实提供该方法。本示例使用 AntiWebUI 原有 API，
没有引入 GPUI 类型。

## 验证

```sh
cargo test --locked --manifest-path examples/anti_webui/Cargo.toml
cargo clippy --locked --manifest-path examples/anti_webui/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path examples/anti_webui/Cargo.toml --check
```

测试挂载真实 `Ui`，完成布局后发送指针按下/释放事件，检查 signal、文字、筛选
结果和条件 children 的变化。测试本身不创建 GPU 或桌面窗口。

## 依赖版本

这是独立 Cargo workspace，使用自己的 `Cargo.lock`，不将 AntiWebUI 的依赖加入
主库或 GPUI 示例。它不包含在根目录的 `cargo test --workspace` 中，需要运行上述命令。

AntiWebUI 固定在 [`f8035a5`](https://github.com/bea4dev/AntiWebUI/tree/f8035a5f0f7d439d2bebe3479e239c4e6254c626)。
上游使用定制 wgpu；Cargo 不继承 Git 依赖中的 `[patch.crates-io]`，因此这里显式
保留上游的补丁，并固定到其 Cargo.lock 中的 `ff143ae` 提交。
