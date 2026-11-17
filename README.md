# gds-uim

用 Rust 宏描述构建器界面，让布局、样式和子元素结构写在一起。支持 GPUI 和 AntiWebUI。

`el!` 构建元素树，`m!` 将一组属性变成构建器闭包，便于共享样式。宏展开为普通
Rust 调用，类型、借用和所有权遵循 Rust 规则。元素和组件由所用 UI 框架或你的项目提供。

```rust
use gds_uim::el;
use gpui::{div, prelude::*, px, rgb};

let element = el! {
    .div.{
        # "welcome";
        flex; flex_col; gap: 12.0.px; p: 24.0.px;
        rounded_lg; bg: 0x1e293b.rgb; text_color: 0xe2e8f0.rgb;
    }.[
        .div.{ text_size: 24.0.px }.["Hello, GPUI"],
        .div.{ text_size: 14.0.px }.["Build your interface with Rust."],
    ]
};
```

## 接入项目

在已有 GPUI 项目的 `Cargo.toml` 中添加 Git 依赖：

```toml
[dependencies]
gds-uim = { git = "https://github.com/GanDesktop/gds-uim.git" }
```

包名是 `gds-uim`，Rust 中通过 `gds_uim` 导入。使用时引入 `el`、按需引入
`m`，以及构建器方法所需的 trait，通常是 `gpui::prelude::*`。
仓库使用 Rust 2024，示例基于 GPUI 0.2.2；主库仅依赖过程宏 crate。

## 核心写法

表达式通过 `.{ 属性 }` 和 `.[ 子元素 ]` 应用映射，点号不可省略。`.` 将左侧结果传给右侧函数：

| UIM | 对应的 Rust 调用 |
| --- | --- |
| `.div` | `div()` |
| `8.0.px` | `px(8.0)` |
| `"ok".Button::with_label` | `Button::with_label("ok")` |
| `(a, b).function` | `function(a, b)` |
| `element.{ flex; gap: 8.0.px }` | `element.flex().gap(px(8.0))` |
| `element.[title, body]` | `element.child(title).child(body)` |
| `element.[..items]` | `element.children(items)` |
| `~label` | `label.clone()` |

裸标识符表示已有值。属性用 `;` 分隔，参数和子元素用 `,` 分隔，末尾分隔符
可以省略。属性块、子元素列表和函数后缀可以连续组合，按书写顺序执行。

字段访问使用 `->`，例如 `if task->priority [...]`。计算通过 `$` 使用 Rust 表达式；
需要继续接 UIM 后缀时，用 `$(...)` 明确边界。宏调用可以直接写：

```text
task->title                   字段访问，等价于 $(task.title)
$(spacing + 4.0).px            计算后调用 px
format!("Task {id}")           直接使用 Rust 宏
```

`#` 设置元素 ID，`#?` 为调试选择器自动补上零参数闭包：

```text
# "task-list";                           → id: "task-list";
#? move format!("task-{id}");             → debug_selector: move () => format!("task-{id}");
```

Rust 2024 要求 `#` 与字符串之间留空格。调试选择器返回 `String`，以下两种
转换写法等价：

```text
#? "empty-state".String::from;
#? "empty-state".{ into };
```

## 样式与闭包

值位置的 `{ ... }` 是一个配置构建器的闭包，可直接传给 `hover`、`when`
等方法。需要移动捕获时写 `move { ... }`。在普通 Rust 中，用 `m!` 创建同样的闭包：

```rust
use gds_uim::{el, m};
use gpui::{div, prelude::*, px, rgb};

let selected = true;
let element = el! {
    .div.{
        # "button";
        bg: 0x6366f1.rgb;
        hover: { bg: 0x818cf8.rgb };
        when: selected, { border_1; border_color: 0xffffff.rgb };
    }.["Select"]
};

let padded = div().map(m! { p: 12.0.px; rounded_lg });
```

显式参数闭包使用 `=>`，例如 `item => .div.[item]`、`(a, b) => (a, b).combine`
和 `() => expr`。移动捕获写 `move (a, b) => expr`；单个元组参数写 `((a, b),) => expr`。

## 条件与列表

`if` 和 `match` 可以直接选择要加入的子元素，`..items` 展开已有集合：

```rust
use gds_uim::el;
use gpui::{div, prelude::*};

let loading = false;
let items = ["First task", "Second task"];
let status: Result<&str, &str> = Ok("Ready");

let element = el! {
    .div.{ flex; flex_col }.[
        if loading [
            "Loading…",
        ] else [
            ..items,
        ],
        match status ^(
            Ok(message) => [message],
            Err(error) => [.div.[format!("Error: {error}")]],
        ),
    ]
};
```

连续条件分支写 `else if condition`，条件和匹配值使用 UIM 表达式，
Rust 运算使用 `$(...)`；模式直接使用 Rust 语法。match 分支列表用 `^(...)` 包围。
属性、children 映射中的 `if` 可以省略 `else`，未命中时保持 builder 不变。
单值分支写 `^(expr)`，必须有 `else`：

```text
if ready ^(.div.["Ready"]) else ^(.div.["Waiting"])
```

属性块和 children 块可以嵌套分组，不会增加父元素：

```text
.div.{ flex; { flex_col; gap: 8.0.px }; p: 12.0.px }.["first", ["second", "third"]
```

元组写 `@(a, b)`，`@(a)` 是单元素元组；支持 `!`、`&`、`&mut`、`*`、`-` 和 `~` 单目运算。
普通构造调用直接写 `.factory` 或 `(a, b).factory`，无需 Rust 转义。

列表筛选、排序等数据处理可以在宏外使用普通 Rust 迭代器。构建子元素时也可以
写 `..items.{ into_iter; map: render_item }`，其中 `render_item` 接收一个集合元素。

## 组件与事件

`#[component]` 为带 `#[attr]` 的字段生成链式 setter，构造器继续手写。
支持 `into`、`any_element` 元素转换、`option`、可选值透传、方法重命名和可见性配置，完整用法见
[组件属性](docs/attr.md)。例如：

```rust
use gds_uim::{component, el};

#[component]
struct Label {
    #[attr(into)]
    text: String,
}

impl Label {
    fn new() -> Self {
        Self { text: String::new() }
    }
}

let label = el! { .Label::new.{ text: "Hello" } };
assert_eq!(label.text, "Hello");
```

使用 `gpui-kit` 时可写 `#[component(gpui = gpui_kit)]`，让 `any_element`
使用该库重导出的 GPUI trait。默认的 `#[component]` 使用 `::gpui` 路径。

组件通过构建器方法暴露具名属性，事件逻辑放在成员方法中。下面取自
[task_board](examples/task_board.rs)，`Button` 和 `StatCard` 是该示例定义的组件：

```rust,ignore
el! {
    .div.[
        "reset-board".Button::new.{
            label: "Reset demo";
            on_click: (cx, Self::on_reset).Context::listener;
        },
        "Completed".StatCard::with_label.{
            value: completed;
            color: GREEN;
        },
    ]
}
```

需要捕获任务 ID、筛选条件等数据时，可以让 `Self::on_select_task(id)` 返回
回调，再写 `(cx, id.Self::on_select_task).Context::listener`。状态修改与通知
保留在普通 Rust 方法中。

## 运行示例

| 示例 | 内容 |
| --- | --- |
| [clicker](examples/clicker.rs) | 计数、重置、基础布局和悬停样式 |
| [task_board](examples/task_board.rs) | 任务筛选、详情、优先级、空状态和自定义组件 |
| [AntiWebUI](examples/anti_webui/README.md) | 使用另一原生 UI 框架实现响应式任务面板 |

在仓库根目录运行：

```sh
cargo run --locked --example clicker
cargo run --locked --example task_board
cargo run --locked --manifest-path examples/anti_webui/Cargo.toml
```

AntiWebUI 示例是独立 Cargo workspace，使用自己的依赖锁定和上游 wgpu 补丁。
它的测试、运行环境及离屏预览命令见 [示例说明](examples/anti_webui/README.md)。

## 文档与开发

完整的语法、求值规则和 EBNF 见 [语法文档](docs/grammer.md)。
展开结构与回归测试范围见 [宏展开阶段](docs/expansion.md)。
给 AI 使用的英文 skill 见 [gds-uim skill](skills/gds-uim/SKILL.md)，涵盖语法、
组件和 GPUI / AntiWebUI 接入规则。
过程宏实现位于 [macros/src](macros/src)，主库在 [src/lib.rs](src/lib.rs) 中导出
`el!`、`m!` 和 `component`。

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all --check
```
