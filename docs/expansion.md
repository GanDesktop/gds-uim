# 宏展开阶段

`el!` 解析完整 UIM 表达式，然后从左到右展开 `.` 应用。v2 的属性映射
`.{ ... }` 和 children 映射 `.[ ... ]` 都要求显式点号。

直接应用的普通映射展开成构建器调用。非空属性块使用局部绑定，保留顺序和
构建器类型变化；普通 children 连续调用 `.child(...)`、`.children(...)`：

```rust
wrap({
    let element = construct();
    let element = element.flex();
    let element = element.gap(spacing);
    element
}.child(first).children(rest))
```

嵌套属性或 children 分组仍作用于同一个 builder，不创建额外元素。
空映射不产生额外绑定。生成的局部变量使用宏卫生标记，不遮蔽调用方同名变量。

映射块中的 `if` / `match` 先绑定父 builder，再更新命中的分支。值分支
`^(expr)` 展开成 Rust 表达式；match 的 `^(arms)` 明确分支列表边界。
条件、匹配值和守卫都先按 UIM 解析，再交给 Rust 检查。匹配值保留 place
expression 的借用语义；Rust 转义中的 let chains 保留短路及模式绑定。

独立映射、`move` 映射、`=>`、`m!` 和 `#?` 按各自语义生成闭包。
全部由映射分支组成的值控制流生成一个闭包，在调用时判断并更新 builder，
而不是返回多个不同类型的 Rust 闭包。

直接应用可省去闭包，但当块内有 `return`、`?` 等控制转移或不透明宏时，
保留闭包边界，避免改变 Rust 控制流的作用域。嵌套分组本身不引入额外闭包。

普通 children 保持连续的构建器表达式，避免逐项 `let` 绑定过早结束临时值
的生命周期。需要跨属性块或条件块复用临时数据时，可以在宏外先绑定数据。
动态可调用值使用带 `FnOnce` 约束的辅助函数，保留参数类型推断、转换和重新借用。
函数路径直接调用，保留 Rust 的高阶回调推断。

一次过程宏调用直接生成嵌套元素树，不逐 token 递归调用辅助宏。
`tests/expansion.rs` 在默认递归上限下构建 32 层嵌套 DSL，builder 无需 Clone。
`tests/el.rs` 检查求值顺序、所有权、类型变化、转义、分组、元组、单目运算、
条件边界与闭包控制流；真实 GPUI 测试和示例覆盖布局、绘制和事件。
