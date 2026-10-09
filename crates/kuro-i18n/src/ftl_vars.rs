//! Variable names a Fluent message reads. Shared by `build.rs` (generated `MsgKey::args`) and the crate.

use fluent_syntax::ast::{
    CallArguments, Expression, InlineExpression, Message, Pattern, PatternElement,
};

/// Sorted, unique variable names (without `$`) read by the message value.
pub fn message_vars(message: &Message<&str>) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(pattern) = &message.value {
        pattern_vars(pattern, &mut out);
    }
    out.sort();
    out.dedup();
    out
}

fn pattern_vars(pattern: &Pattern<&str>, out: &mut Vec<String>) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            expression_vars(expression, out);
        }
    }
}

fn expression_vars(expression: &Expression<&str>, out: &mut Vec<String>) {
    match expression {
        Expression::Select { selector, variants } => {
            inline_vars(selector, out);
            for variant in variants {
                pattern_vars(&variant.value, out);
            }
        }
        Expression::Inline(inline) => inline_vars(inline, out),
    }
}

fn inline_vars(inline: &InlineExpression<&str>, out: &mut Vec<String>) {
    match inline {
        InlineExpression::VariableReference { id } => out.push(id.name.to_string()),
        InlineExpression::FunctionReference { arguments, .. } => call_vars(arguments, out),
        InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => call_vars(arguments, out),
        InlineExpression::Placeable { expression } => expression_vars(expression, out),
        _ => {}
    }
}

fn call_vars(arguments: &CallArguments<&str>, out: &mut Vec<String>) {
    for positional in &arguments.positional {
        inline_vars(positional, out);
    }
    for named in &arguments.named {
        inline_vars(&named.value, out);
    }
}
