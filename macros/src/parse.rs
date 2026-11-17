use proc_macro2::{Span, TokenStream, TokenTree};
use quote::quote;
use syn::{
    Expr, ExprPath, Ident, Lit, Member, Pat, Result, Token, braced, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    token,
};

pub struct Expression {
    pub head: Head,
    pub suffixes: Vec<Suffix>,
}

pub enum Suffix {
    Call(Value),
    Field(Member),
}

pub enum Head {
    Value(Value),
    Call(Vec<Expression>, Value),
}

pub enum Value {
    Rust(Box<Expr>),
    Field(Box<Value>, Member),
    Unary(TokenStream, Box<Value>),
    Clone(Token![~], Box<Value>),
    Tuple(Span, Vec<Expression>),
    Mapper(Box<Mapper>),
    Closure(Box<Closure>),
    Control(Box<Control>),
}

impl Value {
    pub fn span(&self) -> Span {
        match self {
            Self::Rust(value) => value.span(),
            Self::Field(_, member) => member.span(),
            Self::Unary(op, _) => op.span(),
            Self::Clone(tilde, _) => tilde.span(),
            Self::Tuple(span, _) => *span,
            Self::Mapper(mapper) => mapper.span,
            Self::Closure(closure) => closure.arrow.span(),
            Self::Control(control) => control.span,
        }
    }
}

pub struct Closure {
    pub capture: Option<Token![move]>,
    pub inputs: Punctuated<Pat, Token![,]>,
    pub arrow: Token![=>],
    pub body: Expression,
}

pub struct Control {
    pub span: Span,
    pub kind: ControlKind,
}

pub enum ControlKind {
    If {
        branches: Vec<(Expr, Branch)>,
        otherwise: Option<Box<Branch>>,
    },
    Match {
        value: Box<Expr>,
        arms: Vec<Arm>,
    },
}

pub struct Arm {
    pub pattern: Pat,
    pub guard: Option<Expr>,
    pub body: Branch,
}

pub enum Branch {
    Mapping(Section),
    Value(Expression),
}

pub enum Section {
    Attributes(Attributes),
    Children(Children),
}

#[derive(Default)]
pub struct Attributes(pub Vec<Attribute>);

pub enum Attribute {
    Method(Ident, Vec<Expression>),
    Id(Token![#], Expression),
    DebugSelector(Token![#], Option<Token![move]>, Expression),
    Group(Section),
    Control(Box<Control>),
}

pub struct Mapper {
    pub span: Span,
    pub capture: Option<Token![move]>,
    pub section: Section,
}

pub struct Children(pub Vec<Child>);

pub enum Child {
    Element {
        spread: Option<Token![..]>,
        expression: Expression,
    },
    Group(Section),
    Control(Box<Control>),
}

fn control_ahead(input: ParseStream) -> bool {
    input.peek(Token![if]) || input.peek(Token![match])
}

fn closure_ahead(input: ParseStream) -> bool {
    let fork = input.fork();
    if fork.peek(Token![move]) {
        let _ = fork.parse::<Token![move]>();
    }
    if fork.peek(token::Paren) || fork.peek(Ident) || fork.peek(Token![_]) {
        let _ = fork.parse::<TokenTree>();
        fork.peek(Token![=>])
    } else {
        false
    }
}

impl Parse for Expression {
    fn parse(input: ParseStream) -> Result<Self> {
        let head = if input.peek(token::Paren) && !closure_ahead(input) {
            let content;
            parenthesized!(content in input);
            let arguments = arguments(&content)?;
            if !input.peek(Token![.]) || input.peek(Token![..]) {
                return Err(input.error("expected .callable after an argument list or => after closure parameters; use @(...) for a tuple"));
            }
            input.parse::<Token![.]>()?;
            Head::Call(arguments, Value::atom(input)?)
        } else if input.peek(Token![.]) && !input.peek(Token![..]) {
            input.parse::<Token![.]>()?;
            Head::Call(Vec::new(), Value::atom(input)?)
        } else {
            Head::Value(input.parse()?)
        };
        let mut suffixes = Vec::new();
        loop {
            if input.peek(Token![.]) && !input.peek(Token![..]) {
                input.parse::<Token![.]>()?;
                suffixes.push(Suffix::Call(Value::atom(input)?));
            } else if input.peek(Token![->]) {
                input.parse::<Token![->]>()?;
                suffixes.push(Suffix::Field(input.parse()?));
            } else {
                break;
            }
        }
        // The caller owns the boundary: a condition ends before its branch,
        // while arguments, attributes and children require their own separator.
        Ok(Self { head, suffixes })
    }
}

impl Parse for Value {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut value = Self::atom(input)?;
        // Field projection binds inside unary operands: ~task->label clones
        // the field, not the task. After a call, -> projects its result instead.
        while input.peek(Token![->]) {
            input.parse::<Token![->]>()?;
            value = Self::Field(Box::new(value), input.parse()?);
        }
        Ok(value)
    }
}

impl Value {
    fn atom(input: ParseStream) -> Result<Self> {
        if input.peek(Token![~]) {
            return Ok(Self::Clone(input.parse()?, Box::new(input.parse()?)));
        }
        if input.peek(Token![!])
            || input.peek(Token![-])
            || input.peek(Token![*])
            || input.peek(Token![&])
        {
            let op: TokenTree = input.parse()?;
            let mutable = if matches!(&op, TokenTree::Punct(p) if p.as_char() == '&') {
                input.parse::<Option<Token![mut]>>()?
            } else {
                None
            };
            return Ok(Self::Unary(
                quote! { #op #mutable },
                Box::new(input.parse()?),
            ));
        }
        if closure_ahead(input) {
            return Ok(Self::Closure(Box::new(input.parse()?)));
        }
        if input.peek(token::Brace) || input.peek(token::Bracket) || input.peek(Token![move]) {
            return Ok(Self::Mapper(Box::new(Mapper::block(input)?)));
        }
        if input.peek(Token![@]) {
            let at: Token![@] = input.parse()?;
            let content;
            parenthesized!(content in input);
            return Ok(Self::Tuple(at.span, arguments(&content)?));
        }
        if control_ahead(input) {
            let control: Control = input.parse()?;
            let mapping = control.is_mapping();
            if let ControlKind::If {
                otherwise: None, ..
            } = &control.kind
                && !mapping
            {
                return Err(syn::Error::new(
                    control.span,
                    "if used as a value requires else",
                ));
            }
            if !mapping && control.has_mapping() {
                return Err(syn::Error::new(
                    control.span,
                    "cannot mix mapping and value branches in a value control",
                ));
            }
            return Ok(Self::Control(Box::new(control)));
        }
        if input.peek(Token![$]) {
            input.parse::<Token![$]>()?;
            if !input.peek(token::Paren) {
                return Ok(Self::Rust(Box::new(input.parse()?)));
            }
            let content;
            parenthesized!(content in input);
            let tokens: TokenStream = content.parse()?;
            return Ok(Self::Rust(Box::new(syn::parse2(quote! { (#tokens) })?)));
        }
        if input.peek(Lit) {
            let literal: Lit = input.parse()?;
            return Ok(Self::Rust(Box::new(syn::parse2(quote! { #literal })?)));
        }
        if input.peek(Ident)
            || input.peek(Token![::])
            || input.peek(Token![<])
            || input.peek(Token![self])
            || input.peek(Token![Self])
            || input.peek(Token![super])
            || input.peek(Token![crate])
        {
            let path: ExprPath = input.parse()?;
            if input.peek(Token![!]) {
                let bang: Token![!] = input.parse()?;
                let group: TokenTree = input.parse()?;
                if !matches!(group, TokenTree::Group(_)) {
                    return Err(syn::Error::new(
                        group.span(),
                        "expected a macro argument group after !",
                    ));
                }
                return Ok(Self::Rust(Box::new(syn::parse2(
                    quote! { #path #bang #group },
                )?)));
            }
            return Ok(Self::Rust(Box::new(Expr::Path(path))));
        }
        Err(input.error(
            "expected a UIM value, mapper, closure, if/match, @(tuple), or $(Rust expression)",
        ))
    }
}

fn closure_parameter(input: ParseStream) -> Result<Pat> {
    let pattern = Pat::parse_single(input)?;
    if input.peek(Token![:]) {
        Ok(Pat::Type(syn::PatType {
            attrs: Vec::new(),
            pat: Box::new(pattern),
            colon_token: input.parse()?,
            ty: input.parse()?,
        }))
    } else {
        Ok(pattern)
    }
}

impl Parse for Closure {
    fn parse(input: ParseStream) -> Result<Self> {
        let capture = input.parse()?;
        let inputs = if input.peek(token::Paren) {
            let content;
            parenthesized!(content in input);
            content.parse_terminated(closure_parameter, Token![,])?
        } else {
            let mut parameters = Punctuated::new();
            parameters.push(Pat::parse_single(input)?);
            parameters
        };
        Ok(Self {
            capture,
            inputs,
            arrow: input.parse()?,
            body: input.parse()?,
        })
    }
}

fn uim_expression(input: ParseStream) -> Result<Expr> {
    syn::parse2(crate::expand::expression(&input.parse()?))
}

fn condition(input: ParseStream) -> Result<Expr> {
    if input.peek(Token![let]) {
        let let_token: Token![let] = input.parse()?;
        let pattern = Pat::parse_multi_with_leading_vert(input)?;
        let eq: Token![=] = input.parse()?;
        let value = uim_expression(input)?;
        return syn::parse2(quote! { #let_token #pattern #eq #value });
    }
    // Rust escapes may contain a whole let-chain. Its outer parentheses are
    // only an escape boundary, not parentheses around a Rust let condition.
    let mut expr = uim_expression(input)?;
    while let Expr::Paren(paren) = expr {
        expr = *paren.expr;
    }
    Ok(expr)
}

impl Control {
    fn bodies(&self) -> Vec<&Branch> {
        match &self.kind {
            ControlKind::If {
                branches,
                otherwise,
            } => branches
                .iter()
                .map(|(_, b)| b)
                .chain(otherwise.as_deref())
                .collect(),
            ControlKind::Match { arms, .. } => arms.iter().map(|a| &a.body).collect(),
        }
    }
    pub fn is_mapping(&self) -> bool {
        let bodies = self.bodies();
        !bodies.is_empty() && bodies.iter().all(|b| matches!(b, Branch::Mapping(_)))
    }
    fn has_mapping(&self) -> bool {
        self.bodies()
            .iter()
            .any(|b| matches!(b, Branch::Mapping(_)))
    }
}

impl Parse for Control {
    fn parse(input: ParseStream) -> Result<Self> {
        let span = input.span();
        let kind = if input.peek(Token![if]) {
            input.parse::<Token![if]>()?;
            let mut branches = vec![(condition(input)?, input.parse()?)];
            let mut otherwise = None;
            while input.peek(Token![else]) {
                input.parse::<Token![else]>()?;
                if input.peek(Token![if]) {
                    input.parse::<Token![if]>()?;
                    branches.push((condition(input)?, input.parse()?));
                } else {
                    otherwise = Some(Box::new(input.parse()?));
                    break;
                }
            }
            ControlKind::If {
                branches,
                otherwise,
            }
        } else {
            input.parse::<Token![match]>()?;
            let value = Box::new(uim_expression(input)?);
            input.parse::<Token![^]>()?;
            let content;
            parenthesized!(content in input);
            let mut arms = Vec::new();
            while !content.is_empty() {
                let pattern = Pat::parse_multi_with_leading_vert(&content)?;
                let guard = if content.peek(Token![if]) {
                    content.parse::<Token![if]>()?;
                    let mut tokens = TokenStream::new();
                    while !content.is_empty() && !content.peek(Token![=>]) {
                        tokens.extend([content.parse::<TokenTree>()?]);
                    }
                    Some(syn::parse2(crate::expand::expression(&syn::parse2::<
                        Expression,
                    >(
                        tokens
                    )?))?)
                } else {
                    None
                };
                content.parse::<Token![=>]>()?;
                arms.push(Arm {
                    pattern,
                    guard,
                    body: content.parse()?,
                });
                if !content.is_empty() {
                    content.parse::<Token![,]>()?;
                }
            }
            ControlKind::Match { value, arms }
        };
        Ok(Self { span, kind })
    }
}

impl Parse for Branch {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Token![^]) {
            input.parse::<Token![^]>()?;
            let content;
            parenthesized!(content in input);
            let expression = content.parse()?;
            if !content.is_empty() {
                return Err(content.error("expected one UIM expression inside ^(...)"));
            }
            Ok(Self::Value(expression))
        } else {
            Ok(Self::Mapping(input.parse()?))
        }
    }
}

impl Parse for Section {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(token::Brace) {
            let content;
            braced!(content in input);
            Ok(Self::Attributes(content.parse()?))
        } else if input.peek(token::Bracket) {
            let content;
            bracketed!(content in input);
            Ok(Self::Children(content.parse()?))
        } else {
            Err(input.error(
                "expected an attribute block {...}, children block [...], or value branch ^(...)",
            ))
        }
    }
}

impl Parse for Children {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut children = Vec::new();
        while !input.is_empty() {
            children.push(if control_ahead(input) {
                let expression: Expression = input.parse()?;
                match expression {
                    Expression {
                        head: Head::Value(Value::Control(control)),
                        suffixes,
                    } if control.is_mapping() && suffixes.is_empty() => Child::Control(control),
                    expression => Child::Element {
                        spread: None,
                        expression,
                    },
                }
            } else if input.peek(token::Brace) || input.peek(token::Bracket) {
                Child::Group(input.parse()?)
            } else {
                Child::Element {
                    spread: input.parse()?,
                    expression: input.parse()?,
                }
            });
            if !input.is_empty() {
                input.parse::<Token![,]>().map_err(|_| {
                    input.error("expected , between children; use . before a mapping block")
                })?;
            }
        }
        Ok(Self(children))
    }
}

fn arguments(input: ParseStream) -> Result<Vec<Expression>> {
    let mut args = Vec::new();
    while !input.is_empty() && !input.peek(Token![;]) {
        args.push(input.parse()?);
        if !input.is_empty() && !input.peek(Token![;]) {
            input.parse::<Token![,]>()?;
        }
    }
    Ok(args)
}

impl Parse for Attributes {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut attributes = Vec::new();
        while !input.is_empty() {
            attributes.push(input.parse()?);
            if !input.is_empty() {
                input.parse::<Token![;]>().map_err(|_| {
                    input.error("expected ; between attributes; use . before a mapping block")
                })?;
            }
        }
        Ok(Self(attributes))
    }
}

impl Parse for Attribute {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Token![#]) {
            let hash = input.parse()?;
            if input.peek(Token![?]) {
                input.parse::<Token![?]>()?;
                return Ok(Self::DebugSelector(hash, input.parse()?, input.parse()?));
            }
            return Ok(Self::Id(hash, input.parse()?));
        }
        if control_ahead(input) {
            return Ok(Self::Control(Box::new(input.parse()?)));
        }
        if input.peek(token::Brace) || input.peek(token::Bracket) {
            return Ok(Self::Group(input.parse()?));
        }
        let method = input.parse()?;
        if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            Ok(Self::Method(method, arguments(input)?))
        } else {
            Ok(Self::Method(method, Vec::new()))
        }
    }
}

impl Mapper {
    fn block(input: ParseStream) -> Result<Self> {
        Ok(Self {
            span: input.span(),
            capture: input.parse()?,
            section: input.parse()?,
        })
    }
}

impl Parse for Mapper {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Token![move]) {
            Self::block(input)
        } else {
            Ok(Self {
                span: input.span(),
                capture: None,
                section: Section::Attributes(input.parse()?),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_old_syntax_and_incomplete_v2_forms() {
        for source in [
            "",
            ".",
            "~",
            "&mut",
            "!",
            "#div",
            "List()",
            "(1, 2)",
            "(1;).f",
            "(,1).f",
            "(1,,2).f",
            ".div { flex }",
            ".div [a]",
            ".div.{ flex gap: 1 }",
            ".div.{ flex;; }",
            ".div.[a b]",
            ".div.[a,,b]",
            ".div.[..]",
            ".div,",
            ".div.{ #? }",
            ".div.{ #? move }",
            ".div.{ #?? x }",
            "f! value",
            "$(1 +)",
            "() =>",
            "move (a,) =>",
            "(,a) => a",
            "move value",
            "@if (true) a @else b @end",
            "@(a,,b)",
            "@(a; b)",
            "^(x)",
            "if ready ^a else ^b",
            "if ready ^(a)",
            "if ready ^() else ^(a)",
            "if ready ^(a, b) else ^(c)",
            "if ready ^(a) else ^(b) else ^(c)",
            "if ready ^(a) else {}",
            "match x { _ => ^(a) }",
            "match x ^(_ => ^(a) _ => ^(b))",
            "match x ^(_ => a)",
            ".div.{ {flex} flex_col }",
            ".div.[[a]; b]",
            "task->",
            "task->()",
            "task->priority()",
            "task->-1",
            "task->1.5",
        ] {
            assert!(
                syn::parse_str::<Expression>(source).is_err(),
                "accepted invalid input: {source}"
            );
        }
    }

    #[test]
    fn accepts_v2_boundaries_and_nested_controls() {
        for source in [
            ".div.{}.[]",
            "@(a)",
            "@()",
            "@(1.px, ~label)",
            "&mut v.f",
            "!-v.f",
            "move [~label]",
            ".div.{ {flex; {gap: 1}}; flex_col }. [[a], b]",
            "if ready ^(.div) else ^(.div).decorate",
            "if let Some(x) = value ^(x) else ^(fallback)",
            "match &value ^(Some(x) if $(x > 0) => ^(x), _ => ^(fallback),)",
            "match a ^(A => ^(match b ^(B => ^(x), _ => ^(z))), _ => ^(y))",
            ".div.[if ready [a] else [b], match x ^(_ => [c]), [d]]",
            ".div.{ if flag.check { flex }; match value ^(_ => { gap: 1 }) }",
            "if ready { flex } else { flex_col }",
        ] {
            syn::parse_str::<Expression>(source)
                .unwrap_or_else(|e| panic!("rejected {source}: {e}"));
        }
    }

    #[test]
    fn mappers_require_complete_blocks_and_separators() {
        for source in [
            "move",
            "move {} extra",
            "flex gap: 1",
            "map =>",
            "#",
            "#?",
            "#? move",
        ] {
            assert!(
                syn::parse_str::<Mapper>(source).is_err(),
                "accepted mapper: {source}"
            );
        }
        for source in [
            "",
            "flex;",
            "move {}",
            "move []",
            "{flex}; gap: 1",
            "#? move format!(\"task\"); gap: 1",
        ] {
            assert!(
                syn::parse_str::<Mapper>(source).is_ok(),
                "rejected mapper: {source}"
            );
        }
    }
}
