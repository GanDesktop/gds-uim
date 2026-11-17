---
name: gds-uim
description: Write, migrate, or review Rust UI code using gds-uim's el!, m!, and component macros. Use for UIM syntax, builder composition, and custom components in GPUI or AntiWebUI projects.
---

# Author UIM interfaces

Use the current UIM v2 syntax. UIM expands to ordinary Rust calls; the chosen UI
framework supplies builders, methods, traits, rendering, and state management.
Preserve the project's framework and component conventions.

## Read the relevant source

In this repository, [the grammar](../../docs/grammer.md) is the canonical language
reference. Read [component attributes](../../docs/attr.md) when defining props,
and [expansion rules](../../docs/expansion.md) when changing macro behavior.
Consult [GPUI components](../../examples/task_board.rs) or the
[AntiWebUI example](../../examples/anti_webui/README.md) for the selected backend.
These links are relative to this skill's directory in the gds-uim repository;
when using a copied skill, find the matching gds-uim checkout or dependency source.
Check the consuming project's dependency versions before choosing framework APIs.

## Calls, methods, and fields

Import `gds_uim::{el, m, component}` as needed, plus the framework traits used by
the builders. Read each dot as function application, not Rust method lookup:

| UIM | Rust meaning |
| --- | --- |
| `.factory` | `factory()` |
| `value.render` | `render(value)` |
| `(a, b).build` | `build(a, b)` |
| `value.f.g` | `g(f(value))` |
| `value.{ method: a, b }` | `value.method(a, b)` |
| `task->title` | `task.title` |
| `.factory->field` | `factory().field` |
| `~task->title` | `task.title.clone()` |

Bare paths denote existing values. Prefer `.factory` to `$factory()`.
Field projection supports chains, raw identifiers, and tuple indices:
`task->meta->priority`, `value->r#type`, `pair->0`.

## Mappings and children

Always apply blocks with a dot: `builder.{ ... }.[ ... ]`.
Separate attributes with semicolons and children or arguments with commas;
trailing separators are optional. `flag` and `flag:` both call `flag()`.

```uim
.div.{
    flex; flex_col;
    { gap: spacing; p: padding };
}.[
    title,
    [first, second],
    ..remaining,
]
```

`{ ... }` and `[ ... ]` are logical builder-to-result mappings. An attribute such
as `gap: spacing` has the same mapping role within an attribute block; wrap it
as `.{ gap: spacing }` to apply it after a dot. Nested groups act on the same
builder and add no wrapper element. `child` expressions call `.child(value)`;
`..items` calls `.children(items)`. Empty mappings are identity operations.
Methods may change the builder type, subject to Rust's type rules.

Mapping values can be passed as closures, for example `hover: { bg: color }`.
Use `move { ... }` or `move [ ... ]` for move captures. In ordinary Rust,
`m! { p: padding }` creates an attribute mapping; `m!(move { ... })` moves captures.
Direct application may inline the mapping; do not assume it allocates a closure.

## Control flow boundaries

Conditions, match subjects, and guards are UIM expressions. Patterns use Rust
syntax. Mapping branches update the current builder; their `else` is optional:

```uim
.div.{
    if task->priority { bg: highlight };
    if let Some(width) = optional_width { w: width };
}.[
    if loading [spinner] else [content],
    match result ^(
        Ok(item) => [item],
        Err(error) => [error.render_error],
    ),
]
```

Single-value branches require `^(expression)` and value `if` requires `else`.
The entire match arm list also uses `^(...)`, with comma-separated arms:

```uim
if task->done ^("Done") else ^("Open")
match task->label ^(
    Some(label) => ^(label),
    None => ^(fallback),
)
```

Do not mix mapping and value branches in a value control. Only the selected
branch executes; match must be exhaustive. Branch result types must agree.
A suffix outside the final branch applies to the whole control result:
`if ready ^(yes) else ^(no).decorate` decorates either result;
`if ready ^(yes) else ^(no.decorate)` decorates only `no`.
Keep `{ ... }` for attribute mappings and `[ ... ]` for child mappings; neither
means a general Rust block. Do not emit legacy `@if`, `@match`, `@case`, or `@end`,
implicit block suffixes, bare `^value`, or a Rust-style `match value { arms }` list.

## Rust escapes, tuples, and callbacks

Expression positions use UIM unless escaped with `$`. Use `$(expr)` for indexing,
arithmetic, comparisons, or other Rust expressions, and `${ ... }` for Rust
statements. Delimit escapes before returning to UIM: `$(width + 4.0).px`.
Use `->` for ordinary field access. Macro invocations such as `format!("{id}")`
are valid values; their contents follow the called macro's syntax. Types and
patterns also retain Rust syntax. Rust let chains need an escape and edition 2024.

Distinguish the parentheses forms:

| Form | Meaning |
| --- | --- |
| `(a, b).f` | Two call arguments |
| `@(a, b).f` | One tuple argument |
| `@(a)` / `@()` | A one-element tuple / unit |
| `$(expr)` | A Rust expression |
| `^(expr)` | A control-flow value branch |
| `(a, b) => expr` | A two-parameter closure |

Ordinary `(expr)` is not standalone grouping: a parenthesized argument list must
be followed by `.callable`. For example, `(if ready ^(a) else ^(b)).render` is valid.
Arrow closures use UIM bodies: `item => item.render`, `move () => state.read`, or
`move (event, window, cx) => ${ /* Rust event handler */ }`. Use typed parameters
when inference needs help, and `((a, b),) => expr` for one tuple parameter.

Unary `!`, `&`, `&mut`, `*`, `-`, and `~` bind before dot application; field access
binds inside unary operands. `~` clones explicitly and never clones captures
automatically. Retain Rust's moves and borrows; do not add clones to every child.

`# value` calls `id(value)`. `#? expr` calls `debug_selector(|| expr)`;
`#? move expr` moves captures. Use them only on builders with those methods.
Leave a space between `#` and a string literal for Rust 2024.

## Components and framework integration

`#[component]` generates consuming setters for annotated named struct fields:

```rust
#[component]
struct Label {
    #[attr(into)]
    text: String,
    #[attr(option)]
    width: Option<f32>,
}
```

`#[attr]` takes the field type, `into` accepts `impl Into<T>`, and `option` accepts
the inner type and stores `Some(value)`. Neither initialization nor a constructor
is generated. `option_setter = "width_opt"` adds a setter taking the full Option;
`name` and `vis` customize the generated method. Keep constructor defaults,
rendering, child insertion, callback wrapping, and style delegation in Rust.

For **GPUI**, `#[attr(any_element)]` accepts `IntoElement` and stores `AnyElement`.
It can combine with `option`, but not `into`. The default GPUI path is `::gpui`;
`#[component(gpui = gpui_kit)]` selects a compatible re-export. Use the framework's
actual event signatures and rendering traits, following the surrounding code.

For **AntiWebUI**, ordinary `attr`, `into`, and `option` already work without GPUI.
Implement `IntoView` for custom components; implement `Styled` when style methods
should delegate to a root builder. No `component(anti_webui)` or `attr(any_view)`
option exists. Use the backend's style units and event signatures, not GPUI types.
`Binding<T>` can accept `Signal<T>` through `#[attr(into)]`; it does not imply an
automatic conversion from plain `T`.

Read signals inside dynamic text, `dyn_style`, or `dyn_view` callbacks to track
updates. Ordinary UIM conditions have no automatic reactive subscription.
AntiWebUI's `View` is a created node handle: `into_view()` associates new nodes
with the current reactive owner, and `Div::child()` invokes that conversion
immediately. Preserve the component's intended creation time. Deferred slots
can store builders or factories; rebuilt subtrees should create fresh views
inside the reactive callback rather than reuse handles from a disposed owner.

## Validate the change in its actual workspace

For UI changes, compile the affected target and run its relevant existing tests.
For macro changes, preserve evaluation order, single evaluation of inputs,
closure capture, type changes, and the scope of `return` and `?`. Use the existing
parser, expansion, and behavioral tests rather than matching generated wording.

Repository checks are `cargo test --locked --workspace`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`, and
`cargo fmt --all --check`. The AntiWebUI example is a separate workspace; use
`--manifest-path examples/anti_webui/Cargo.toml` for its tests, builds, and formatting.
Its pinned dependencies and wgpu patches stay in that workspace. Documentation-only
changes need reference and example checks, not a full native UI rebuild.
