use std::collections::HashSet;

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{format_ident, quote, quote_spanned};
use syn::Ident;

use crate::parse::{
    Attribute, Attributes, Branch, Child, Children, Control, ControlKind, Expression, Head, Mapper,
    Section, Suffix, Value,
};

pub fn expression(expr: &Expression) -> TokenStream {
    let head = match &expr.head {
        Head::Value(val) => value(val),
        Head::Call(args, callable) => apply(args.iter().map(expression).collect(), callable),
    };
    let mut result = head;
    for suffix in &expr.suffixes {
        result = match suffix {
            Suffix::Call(callable) => apply(vec![result], callable),
            Suffix::Field(member) => quote! { (#result).#member },
        };
    }
    result
}

fn value(val: &Value) -> TokenStream {
    match val {
        Value::Rust(expr) => quote! { #expr },
        Value::Field(base, member) => {
            let base = value(base);
            quote! { (#base).#member }
        }
        Value::Unary(op, inner) => {
            let inner = value(inner);
            quote! { #op (#inner) }
        }
        Value::Tuple(_, items) => {
            let items = items.iter().map(expression);
            quote! { (#(#items,)*) }
        }
        Value::Clone(tilde, inner) => {
            let inner = value(inner);
            quote_spanned! { tilde.span => (#inner).clone() }
        }
        Value::Mapper(mapping) => mapper(mapping),
        Value::Closure(closure) => {
            let capture = &closure.capture;
            let inputs = &closure.inputs;
            let body = expression(&closure.body);
            quote! { #capture |#inputs| { #body } }
        }
        Value::Control(branches) => {
            if branches.is_mapping() {
                let receiver = binding();
                let result = mapping_control(branches, quote! { #receiver });
                quote! { |#receiver| #result }
            } else {
                control(
                    branches,
                    |branch| match branch {
                        Branch::Value(expr) => expression(expr),
                        Branch::Mapping(_) => unreachable!("validated by parser"),
                    },
                    TokenStream::new(),
                )
            }
        }
    }
}

fn apply(args: Vec<TokenStream>, callable: &Value) -> TokenStream {
    let span = callable.span();
    // Immediate mapping application can be emitted as a builder chain. This
    // retains contextual typing, temporary lifetimes and shallow expansions.
    if args.len() == 1 {
        if let Value::Mapper(mapping) = callable
            && mapping.capture.is_none()
        {
            let mapped = section(args[0].clone(), &mapping.section);
            if !needs_closure_scope(mapped.clone()) {
                return mapped;
            }
        }
        if let Value::Control(branches) = callable
            && branches.is_mapping()
        {
            let receiver = binding();
            let input = &args[0];
            let selected = mapping_control(branches, quote! { #receiver });
            if !needs_closure_scope(selected.clone()) {
                return quote! {{ let #receiver = #input; #selected }};
            }
        }
    }
    // Preserve Rust's contextual typing for generic factories taking callbacks
    // (including higher-ranked lifetimes), and its borrowing of Fn/FnMut paths.
    if let Value::Rust(expr) = callable
        && matches!(expr.as_ref(), syn::Expr::Path(_))
    {
        return quote_spanned! { span => #expr(#(#args),*) };
    }
    let callable = value(callable);
    if args.is_empty() {
        return quote_spanned! { span => (#callable)() };
    }
    let types: Vec<_> = (0..args.len()).map(|i| format_ident!("A{i}")).collect();
    let names: Vec<_> = (0..args.len()).map(|i| format_ident!("arg{i}")).collect();
    // Check the callable before its arguments so a factory's fixed signature
    // supplies coercion sites. The FnOnce bound also types inferred closures.
    let helper = helper_name(&args, &callable);
    quote_spanned! { span =>
        {
            #[inline]
            fn #helper<#(#types,)* R>(
                callable: impl ::core::ops::FnOnce(#(#types),*) -> R,
                args: (#(#types,)*),
            ) -> R {
                let (#(#names,)*) = args;
                callable(#(#names),*)
            }
            #helper(#callable, (#(#args,)*))
        }
    }
}

// An immediate application may omit its closure only if Rust control transfers
// cannot observe that omission. Macro bodies are conservatively scanned too.
fn needs_closure_scope(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Ident(ident) => {
            matches!(ident.to_string().as_str(), "return" | "break" | "continue")
        }
        // A macro can expand to a return/try that is invisible at this stage.
        TokenTree::Punct(punct) => matches!(punct.as_char(), '?' | '!'),
        TokenTree::Group(group) => needs_closure_scope(group.stream()),
        _ => false,
    })
}

// Mixed-site spans protect local bindings, but not item names. Avoid capturing
// a caller's function or variable when introducing the local helper function.
fn helper_name(args: &[TokenStream], callable: &TokenStream) -> Ident {
    fn collect(tokens: TokenStream, names: &mut HashSet<String>) {
        for token in tokens {
            match token {
                TokenTree::Ident(ident) => {
                    names.insert(ident.to_string());
                }
                TokenTree::Group(group) => collect(group.stream(), names),
                _ => {}
            }
        }
    }
    let mut names = HashSet::new();
    for tokens in args.iter().chain(std::iter::once(callable)) {
        collect(tokens.clone(), &mut names);
    }
    let mut name = String::from("__uim_apply");
    while names.contains(&name) {
        name.push('_');
    }
    Ident::new(&name, Span::mixed_site())
}

fn binding() -> Ident {
    Ident::new("__uim_value", Span::mixed_site())
}

fn section(result: TokenStream, section: &Section) -> TokenStream {
    match section {
        Section::Attributes(attrs) => attributes(result, attrs),
        Section::Children(items) => children(result, items),
    }
}

fn mapping_control(branches: &Control, receiver: TokenStream) -> TokenStream {
    control(
        branches,
        |branch| match branch {
            Branch::Mapping(mapping) => section(receiver.clone(), mapping),
            Branch::Value(expr) => {
                let result = expression(expr);
                quote! { (#result)(#receiver) }
            }
        },
        receiver.clone(),
    )
}

// Keep ordinary child calls in one expression so temporary inputs live through
// the complete builder chain. Control fragments bind their parent exactly once.
fn children(mut result: TokenStream, items: &Children) -> TokenStream {
    for child in &items.0 {
        result = match child {
            Child::Element {
                spread,
                expression: expr,
            } => {
                let expr = expression(expr);
                if let Some(spread) = spread {
                    quote_spanned! { spread.spans[0] => (#result).children(#expr) }
                } else {
                    quote! { (#result).child(#expr) }
                }
            }
            Child::Group(group) => section(result, group),
            Child::Control(branches) => {
                let receiver = binding();
                let selected = mapping_control(branches, quote! { #receiver });
                quote! {{
                    let #receiver = #result;
                    #selected
                }}
            }
        };
    }
    result
}

fn attributes(initial: TokenStream, attrs: &Attributes) -> TokenStream {
    if attrs.0.is_empty() {
        return initial;
    }
    let receiver = binding();
    let statements = attrs.0.iter().map(|attr| {
        let call = match attr {
            Attribute::Method(method, args) => {
                let args = args.iter().map(expression);
                quote! { #receiver.#method(#(#args),*) }
            }
            Attribute::Group(group) => section(quote! { #receiver }, group),
            Attribute::Control(branches) => mapping_control(branches, quote! { #receiver }),
            Attribute::Id(hash, expr) => {
                let expr = expression(expr);
                quote_spanned! { hash.span => #receiver.id(#expr) }
            }
            Attribute::DebugSelector(hash, capture, expr) => {
                let expr = expression(expr);
                quote_spanned! { hash.span => #receiver.debug_selector(#capture || { #expr }) }
            }
        };
        quote! { let #receiver = #call; }
    });
    quote! {{
        let #receiver = #initial;
        #(#statements)*
        #receiver
    }}
}

// Fragment controls update the existing builder in the selected arm. Keeping
// the match subject intact preserves Rust's borrowing and place-expression
// semantics, and allows different child types without erasing them ourselves.
fn control(
    branches: &Control,
    mut expand: impl FnMut(&Branch) -> TokenStream,
    fallback: TokenStream,
) -> TokenStream {
    match &branches.kind {
        ControlKind::If {
            branches,
            otherwise,
        } => {
            let mut result = otherwise.as_ref().map_or(fallback, |branch| expand(branch));
            for (condition, branch) in branches.iter().rev() {
                let branch = expand(branch);
                // Print in condition context: let chains cannot be wrapped in
                // parentheses, while struct literals may need them.
                let conditional = syn::ExprIf {
                    attrs: Vec::new(),
                    if_token: Default::default(),
                    cond: Box::new(condition.clone()),
                    then_branch: syn::Block {
                        brace_token: Default::default(),
                        stmts: vec![syn::Stmt::Expr(syn::Expr::Verbatim(branch), None)],
                    },
                    else_branch: Some((Default::default(), Box::new(syn::Expr::Verbatim(result)))),
                };
                result = quote! { #conditional };
            }
            result
        }
        ControlKind::Match { value, arms } => {
            let arms = arms.iter().map(|arm| {
                let pattern = &arm.pattern;
                let guard = arm.guard.as_ref().map(|guard| quote! { if #guard });
                let body = expand(&arm.body);
                quote! { #pattern #guard => { #body }, }
            });
            quote! { match (#value) { #(#arms)* } }
        }
    }
}

pub fn mapper(mapping: &Mapper) -> TokenStream {
    let capture = &mapping.capture;
    let receiver = binding();
    let result = section(quote! { #receiver }, &mapping.section);
    quote! { #capture |#receiver| #result }
}
