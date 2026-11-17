# UIM 语法

`el!` 将 UIM 表达式展开成普通 Rust 构建器调用，`m!` 在 Rust 中创建属性映射。
本文统一定义当前已实现的 v2 语法、设计约定和求值规则。
旧版隐式块后缀和 `@if` / `@match` / `@end` 控制指令不再支持。

## 表达式与应用

`.` 将左侧结果传给右侧可调用值，从左到右结合。裸路径表示已有值，
不隐式调用构造器；前导 `.` 表示无参数调用。

```text
.div                 → div()
a.f                  → f(a)
(a, b).f             → f(a, b)
a.f.g                → g(f(a))
```

属性块和 children 块在逻辑上都是单参数映射函数，接收当前 builder，
返回配置后的结果。它们通过 `.` 应用，点号不可省略：

```uim
.div.{
    height: 1.px;
}.[
    a,
    b,
]
```

`value { ... }`、`value [ ... ]` 不再是隐式应用。
`{}` 和 `[]` 都是恒等映射。各阶段按书写顺序应用，允许改变 builder 类型。

逻辑上的函数不要求实现时一定生成 Rust 闭包。直接应用可以展开成构建器
方法调用；独立作为值、参数或被保存时，可以生成闭包。优化须保持求值顺序、
捕获及控制流语义一致，不能只保证最终界面看起来相同。

## 属性、children 与分组

属性之间用 `;`，children 之间用 `,`，最后一个分隔符可以省略。

```uim
.div.{
    attr1;
    attr2: a, b;
    {
        attr3;
        attr4;
    };
    attr5;
}.[
    first,
    [
        second,
        third,
    ],
    ..remaining,
    last,
]
```

属性项 `attr2: a, b` 在逻辑上也是自映射函数：
`builder => builder.attr2(a, b)`。它只能直接写在属性上下文中；若要出现在
`.` 后面，需要用 `{ attr2: a, b }` 包装，以免和其他表达式语法冲突。

嵌套 `{ ... }` 是属性分组，对同一个当前 builder 依次应用内部属性。
嵌套 `[ ... ]` 是 children 分组，对同一个父 builder 依次添加内部 children，
不会创建额外父元素，也不会把分组闭包本身作为 child 添加。

普通 child 表达式对应 `.child(value)`，`..expression` 对应
`.children(expression)`。分组可以递归嵌套，按出现顺序执行。

## 字段访问

`->` 表示 Rust 字段访问，可以用于条件、参数、children 和普通表达式：

```uim
if task->priority ["Priority"]
if !task->done ^("Open") else ^("Done")
task->title.render
```

`task->priority` 等价于 `$(task.priority)`。字段可以连续访问，也支持原始
标识符和元组字段：`task->meta->priority`、`value->r#type`、`pair->0`。
它使用 Rust 的字段规则，支持引用与智能指针的自动解引用，不增加隐式克隆。

字段访问优先于作用在该值上的单目运算；调用之后的字段后缀访问调用结果：

```text
~task->title          → task.title.clone()
&mut task->count      → &mut task.count
!task->priority       → !task.priority
task->title.render    → render(task.title)
.factory->priority    → factory().priority
value.wrap->field     → wrap(value).field
```

字段基值只求值一次。`->` 不用于方法调用；方法仍写成 `value.{ method: args }`。

## UIM 与 Rust 的边界

表达式位置默认使用 UIM。只有 `$` 和 `$()` 内使用 Rust 表达式语法：

```text
value.check                → check(value)
$(value.check())           → Rust 方法调用
$(width + 4).px            → px(width + 4)
${ /* Rust 块 */ }         → Rust 块表达式
```

宏调用的参数仍由对应宏解释。模式和类型位置使用 Rust 模式、类型语法，
不需要 `$`，例如 `if let Some(x) = value` 中的 `Some(x)`。

## 闭包、元组与单目运算

属性块支持 `move { ... }`。显式参数闭包继续使用 `=>`，主体是 UIM 表达式：

```uim
.div.{
    on_click: move (arg1, arg2) => arg1.{
        attr1: arg2;
    };
    on_click2: move (arg1, arg2) => ${
        // 普通 Rust 函数体
    };
}
```

普通圆括号用于调用参数和闭包参数。元组使用 `@(...)`，其中每一项都是
UIM 表达式：

```text
@(a, b)         → 二元组
@(a)            → 单元素元组，等价于 @(a,)
@()             → unit
@(1.px, 2.px)   → (px(1), px(2))
(a, b).f        → f(a, b)
@(a, b).f       → f((a, b))
```

普通 `(expr)` 不是独立的分组表达式；作为调用参数列表时，后面必须接
`.callable`。例如 `(if ready ^(a) else ^(b)).render` 调用 `render`，
而单独包一层 `(if ready ^(a) else ^(b))` 不合法。

支持 `!`、`&`、`&mut`、`*`、`-` 和特殊克隆运算符 `~`。
单目运算作用于紧随其后的值，优先于 `.`：

```text
!v.f       → f(!v)
&v.f       → f(&v)
&mut v.f   → f(&mut v)
*v.f       → f(*v)
-v.f       → f(-v)
~v.f       → f(v.clone())
```

`~` 不隐式克隆闭包捕获的变量。Rust 运算组合可以使用 `$()`。

## 块中的控制流

使用 `if`、`else if`、`else`、`match` 和 `if let`，替代旧版
`@if`、`@case`、`@end`。条件和匹配值是 UIM 表达式，模式直接使用 Rust 语法。

```uim
.div.{
    if cond1 {
        attr1;
        attr2;
    } else if cond2 {
        attr3;
        attr4;
    } else {
        attr5;
    };

    match value ^(
        Some(x) => { attr6: x },
        None => {},
    );

    if let Some(x) = other {
        attr7: x;
    };
}.[
    if cond1 [
        first,
        second,
    ] else [
        fallback,
    ],

    match value ^(
        Some(x) => [x],
        None => [],
    ),

    if let Some(x) = other [
        x,
    ],
]
```

控制流选择对当前 builder 应用的操作。块中的 `if` 可以省略 `else`，
未命中时保持 builder 不变。只执行命中的分支，匹配值只求值一次。
各分支返回的 builder 类型需要兼容。

实现可以把判断放在映射函数内部，并在命中分支更新 builder，
无需构造不同类型的 Rust 闭包再尝试将它们作为同一种类型返回。

## 值控制流

控制流使用 `^(...)` 包围单值分支，内部恰好一个 UIM 表达式。
条件同样使用 UIM 表达式：

```uim
if ready ^(.div.["ready"]) else ^(.div.["waiting"])

if cond1 ^(val1) else if cond2 ^(val2) else ^(val3)

if let Some(x) = value ^(x) else ^(fallback)
```

`^(...)` 是控制流的显式边界，不是元组，也不是 Rust 转义。
不再使用裸 `^expr`；`^` 不作为普通表达式的单目运算符。
块分支仍直接使用 `{ ... }` 或 `[ ... ]`，不需要额外包围 `^(...)`。
值分支需要 `else`；各分支返回类型必须兼容。

即使分支以 `.div` 开头，条件也会在 `^(` 前结束，不会错误地解析成
`ready.div`。不依赖空格、换行或缩进区分表达式。

## match 的分支列表

match 的整个分支列表使用 `^(...)` 包围，分支之间用逗号，末尾逗号可省略。
每个分支体根据含义使用 `{ ... }`、`[ ... ]` 或 `^(expr)`：

```text
match value ^(
    Some(x) => ^(x),
    None => ^(fallback),
)
```

解析器根据当前位置区分 `^(...)` 的内容：match 匹配值后面是分支列表，
单值分支位置是一个表达式。嵌套 match 各自拥有明确结束位置：

```text
match a ^(
    A => ^(match b ^(
        B => ^(x),
        _ => ^(z),
    )),
    _ => ^(y),
)
```

`_ => ^(z)` 属于内层 match，`_ => ^(y)` 属于外层 match，
归属由括号决定，不依赖缩进。

## 控制流的后缀与括号用途

分支括号内的后缀作用于该分支的值；完整控制流结束后的后缀作用于整个
控制流的结果：

```text
if ready ^(yes) else ^(no.decorate)   → 仅对 no 应用 decorate
if ready ^(yes) else ^(no).decorate   → 对选中结果应用 decorate

match value ^(
    Some(x) => ^(x),
    None => ^(fallback),
).decorate
```

各种圆括号形式有各自明确用途：

| 写法 | 含义 |
| --- | --- |
| `(a, b).f` | UIM 调用参数 |
| `(a, b) => expr` | 闭包参数 |
| `@(a, b)` | UIM 元组 |
| `$(expr)` | Rust 表达式转义 |
| `^(expr)` | 控制流的单值分支 |
| `match value ^(arms)` | match 分支列表 |

`{ ... }` 始终用于属性映射，`[ ... ]` 始终用于 children 映射；
Rust 转义和宏参数内部仍由对应的 Rust 或宏语法解释。

## ID、调试选择器与属性参数

`flex` 和 `flex:` 都调用无参数方法；`set_pair: a, b` 调用两个参数的方法。
属性方法名支持 Rust 原始标识符，如 `r#type`。`# value` 等价于 `id: value`。
Rust 2024 要求 `#` 与字符串字面量之间留空格。

`#? expr` 等价于 `debug_selector: () => expr`；`#? move expr` 使用移动捕获。
返回值不隐式转换，GPUI 需要 String 时可写：

```text
# "root";
# @("task", id);
#? "root".String::from;
#? move format!("task-{id}");
```

## Rust 转义、宏和路径

优先使用 UIM 的调用语法：`.factory`、`value.convert`、`(a, b).build`。
字段访问使用 `->`；索引、运算或 Rust 代码块使用 `$(...)` 或 `${ ... }`。
裸 `$rust-expr` 仍接受一个完整 Rust 表达式，但不建议用于普通函数调用。
要从 Rust 返回 UIM 后继续接 `.`，用 `$(...)` 明确边界：

```text
$(state.field).render
$(items[index]).render
$(width + 4.0).px
```

`$(a, b)` 仍可表达 Rust 元组；UIM 元组首选 `@(a, b)`。
`$(...)` 内不解释 UIM，需要时显式嵌套 `el!`。
宏调用本身就是值，支持三种括号与限定路径；宏参数不会被 UIM 重新解释。
路径支持 `::<...>` 和 `<T as Trait>::item`。

数字遵循 Rust 词法。`1.px` 可用；整数后的可调用值如果以符号开头，
在点前留空格，例如 `1 .$(handler)`，避免 `1.` 被读成浮点字面量。

## 模式、守卫与 let chains

`if let Some(x) = value` 的右侧使用 UIM 表达式；模式绑定对命中分支可见。
match 支持 Rust 的解构、`|` 模式及 `if` 守卫，守卫表达式使用 UIM：

```text
match source ^(
    Some(x) if x.is_valid => ^(x.render),
    _ => ^(fallback),
)
```

上面的 `is_valid` 和 `render` 是函数。若要调用方法，写 `x.{ is_valid }`。
复杂条件及 let chains 放进 Rust 转义，并保留 Rust 的短路及绑定规则：

```text
if $(let Some(x) = source && !x.is_empty()) ^(x) else ^(fallback)
```

Rust let chains 要求调用方使用 Rust 2024 edition。match 不自动补默认分支，
穷尽性、借用、移动和类型兼容交给 Rust 检查。

## 求值与闭包

输入表达式只求值一次。普通函数和动态可调用值先求可调用值，再从左到右求参数。
直接应用属性或 children 映射时，先求 builder，再按书写顺序执行操作；
映射中的条件与参数在应用时求值。独立映射作为闭包传递时，捕获发生于创建时。

动态调用保持 `FnOnce` 约束及输入类型推断。需要复用可调用值时，使用
`value.&handler`、`value.&mut handler` 或 `value.~handler`。
固定签名的工厂函数仍能为参数提供重新借用和类型转换上下文。

单参数箭头闭包可以写 `item => expr`，括号内支持 Rust 模式和类型注解，
例如 `(node: Div, width: Pixels) => node.{ w: width }`。
箭头主体是完整 UIM 表达式，向右结合；`(a, b) => { flex }` 返回另一个映射，
不会隐式选择 a 或 b。`((a, b),) => expr` 则只有一个元组参数。
独立保存且调用参数方法的闭包可能需要类型注解。

`m! { flex; gap: 8.0.px }` 在 Rust 中创建属性映射；`m!(move { ... })`
使用移动捕获。映射不会隐式克隆变量；`move { child: ~label }` 在创建时移入
label，在执行时克隆。`when: false, handler` 仍会求值 handler，只跳过调用。

## 组件与 UI 框架

UIM 只生成调用，不提供构建器方法或响应式运行时。示例中的 `div`、`px`、
`child`、`id`、`debug_selector` 等必须由目标框架或项目提供；导入对应的 trait。
同一套语法可用于 GPUI 和 AntiWebUI，但两者的样式类型、事件签名和渲染接口
仍遵循各自 API，不能仅靠换 import 互换。

`#[component]` 为 `#[attr]` 字段生成消费 `self`、返回 `Self` 的 setter。
普通属性、`into` 和 `option` 不依赖框架；`any_element` 是 GPUI 专用转换。
构造器、渲染、children、事件包装和样式委托使用普通 Rust 方法与 trait，
完整规则见 [组件属性](attr.md)。AntiWebUI 自定义组件实现 `IntoView`，
需要通用样式方法时可实现 `Styled`。

UIM 条件在所属表达式或映射执行时求值，不会自动订阅状态。
AntiWebUI 的动态内容需要在动态文本、`dyn_style`、`dyn_view` 等响应式回调中
读取 signal；用法见 [AntiWebUI 示例](../examples/anti_webui/README.md)。

## 形式化语法

`?` 表示可选，`*` 表示重复。模式、类型和宏参数分别交给 Rust 和对应宏。

```text
expression := head ("." callable | "->" member)*
head := value | "." callable | "(" arguments ")" "." callable
arguments := (expression ("," expression)* ","?)?
value := callable ("->" member)*
member := rust-ident | rust-tuple-index
callable := rust-path | rust-literal | rust-macro
       | "$" rust-expression | "@" "(" arguments ")"
       | unary value | "move"? mapping | closure | control
unary := "~" | "!" | "&" | "&" "mut" | "*" | "-"
closure := "move"? closure-params "=>" expression
closure-params := rust-ident | "_" | "(" (closure-param ("," closure-param)* ","?)? ")"
closure-param := rust-pattern (":" rust-type)?
mapping := "{" attributes "}" | "[" children "]"
attributes := (attribute (";" attribute)* ";"?)?
attribute := method (":" arguments)? | "#" expression
           | "#" "?" "move"? expression | mapping | control
method := rust-ident
children := (child ("," child)* ","?)?
child := ".."? expression | mapping | control
control := "if" condition branch ("else" "if" condition branch)* ("else" branch)?
         | "match" expression "^" "(" (arm ("," arm)* ","?)? ")"
condition := expression | "let" rust-pattern "=" expression
arm := rust-pattern ("if" expression)? "=>" branch
branch := mapping | "^" "(" expression ")"
```

单值 if 要求 else；值控制流不能混合映射分支和单值分支。全是映射分支的
控制流可用作映射值，判断在应用时执行。直接出现在 children 中的映射控制流
更新当前父 builder；单值控制流则产生一个 child，可以继续接后缀。

实现与展开回归约束见 [宏展开阶段](expansion.md)。
