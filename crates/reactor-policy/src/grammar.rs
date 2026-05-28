//! Policy expression grammar parser.
//!
//! Parses policy expressions from raw SQL strings.

use crate::ast::{PolicyBinaryOp, PolicyExpr, PolicyExprKind, PolicyLiteral, PolicyUnaryOp};
use crate::builtins::validate_builtin_call;
use thiserror::Error;

/// Errors during policy expression parsing.
#[derive(Debug, Error)]
pub enum PolicyParseError {
    #[error("unexpected end of input")]
    UnexpectedEnd,

    #[error("unexpected token: {0}")]
    UnexpectedToken(String),

    #[error("invalid number: {0}")]
    InvalidNumber(String),

    #[error("unclosed parenthesis")]
    UnclosedParen,

    #[error("unclosed string literal")]
    UnclosedString,

    #[error("invalid auth builtin: {0}")]
    InvalidBuiltin(String),

    #[error("invalid domain builtin: {0}.{1}")]
    InvalidDomainBuiltin(String, String),
}

/// Parse a policy expression from a string.
pub fn parse_policy_expr(input: &str) -> Result<PolicyExpr, PolicyParseError> {
    let mut parser = Parser::new(input);
    let expr = parser.parse_expr()?;

    // Ensure we consumed all input
    parser.skip_whitespace();
    if parser.pos < parser.input.len() {
        return Err(PolicyParseError::UnexpectedToken(
            parser.input[parser.pos..].chars().take(20).collect(),
        ));
    }

    Ok(expr)
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse_expr(&mut self) -> Result<PolicyExpr, PolicyParseError> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> Result<PolicyExpr, PolicyParseError> {
        let mut left = self.parse_and_expr()?;

        loop {
            self.skip_whitespace();
            if self.match_keyword("OR") {
                let right = self.parse_and_expr()?;
                left = PolicyExpr::binary(left, PolicyBinaryOp::Or, right);
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<PolicyExpr, PolicyParseError> {
        let mut left = self.parse_not_expr()?;

        loop {
            self.skip_whitespace();
            if self.match_keyword("AND") {
                let right = self.parse_not_expr()?;
                left = PolicyExpr::binary(left, PolicyBinaryOp::And, right);
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_not_expr(&mut self) -> Result<PolicyExpr, PolicyParseError> {
        self.skip_whitespace();
        if self.match_keyword("NOT") {
            let expr = self.parse_not_expr()?;
            Ok(PolicyExpr::unary(PolicyUnaryOp::Not, expr))
        } else {
            self.parse_comparison()
        }
    }

    fn parse_comparison(&mut self) -> Result<PolicyExpr, PolicyParseError> {
        let left = self.parse_primary()?;

        self.skip_whitespace();

        // Check for IS NULL / IS NOT NULL
        if self.match_keyword("IS") {
            self.skip_whitespace();
            let negated = self.match_keyword("NOT");
            if negated {
                self.skip_whitespace();
            }
            if self.match_keyword("NULL") {
                return Ok(PolicyExpr::is_null(left, negated));
            } else {
                return Err(PolicyParseError::UnexpectedToken(
                    "expected NULL".to_string(),
                ));
            }
        }

        // Check for IN / NOT IN
        let negated_in = self.match_keyword("NOT");
        if negated_in {
            self.skip_whitespace();
        }
        if self.match_keyword("IN") {
            let list = self.parse_list()?;
            return Ok(PolicyExpr::in_list(left, list, negated_in));
        } else if negated_in {
            return Err(PolicyParseError::UnexpectedToken(
                "expected IN after NOT".to_string(),
            ));
        }

        // Check for comparison operators
        if let Some(op) = self.try_parse_comparison_op() {
            let right = self.parse_primary()?;
            return Ok(PolicyExpr::binary(left, op, right));
        }

        Ok(left)
    }

    fn try_parse_comparison_op(&mut self) -> Option<PolicyBinaryOp> {
        self.skip_whitespace();

        // Two-char operators first
        if self.match_str("<=") {
            return Some(PolicyBinaryOp::LtEq);
        }
        if self.match_str(">=") {
            return Some(PolicyBinaryOp::GtEq);
        }
        if self.match_str("<>") || self.match_str("!=") {
            return Some(PolicyBinaryOp::NotEq);
        }

        // Single-char operators
        if self.match_char('=') {
            return Some(PolicyBinaryOp::Eq);
        }
        if self.match_char('<') {
            return Some(PolicyBinaryOp::Lt);
        }
        if self.match_char('>') {
            return Some(PolicyBinaryOp::Gt);
        }

        // Keyword operators
        if self.match_keyword("LIKE") {
            return Some(PolicyBinaryOp::Like);
        }
        if self.match_keyword("ILIKE") {
            return Some(PolicyBinaryOp::ILike);
        }

        None
    }

    fn parse_primary(&mut self) -> Result<PolicyExpr, PolicyParseError> {
        self.skip_whitespace();

        // Parenthesized expression
        if self.match_char('(') {
            let expr = self.parse_expr()?;
            self.skip_whitespace();
            if !self.match_char(')') {
                return Err(PolicyParseError::UnclosedParen);
            }
            return Ok(expr);
        }

        // Unary minus
        if self.match_char('-') {
            let expr = self.parse_primary()?;
            return Ok(PolicyExpr::unary(PolicyUnaryOp::Minus, expr));
        }

        // String literal
        if self.peek() == Some('\'') {
            return self.parse_string_literal();
        }

        // Number
        if self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return self.parse_number();
        }

        // Keywords
        if self.match_keyword("TRUE") {
            return Ok(PolicyExpr::literal(PolicyLiteral::Bool(true)));
        }
        if self.match_keyword("FALSE") {
            return Ok(PolicyExpr::literal(PolicyLiteral::Bool(false)));
        }
        if self.match_keyword("NULL") {
            return Ok(PolicyExpr::literal(PolicyLiteral::Null));
        }

        // Identifier or function call
        self.parse_identifier_or_call()
    }

    fn parse_string_literal(&mut self) -> Result<PolicyExpr, PolicyParseError> {
        if !self.match_char('\'') {
            return Err(PolicyParseError::UnexpectedToken("expected '".to_string()));
        }

        let mut value = String::new();
        loop {
            match self.peek() {
                None => return Err(PolicyParseError::UnclosedString),
                Some('\'') => {
                    self.advance();
                    // Check for escaped quote
                    if self.peek() == Some('\'') {
                        self.advance();
                        value.push('\'');
                    } else {
                        break;
                    }
                }
                Some(c) => {
                    self.advance();
                    value.push(c);
                }
            }
        }

        Ok(PolicyExpr::literal(PolicyLiteral::String(value)))
    }

    fn parse_number(&mut self) -> Result<PolicyExpr, PolicyParseError> {
        let start = self.pos;
        let mut has_dot = false;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
            } else if c == '.' && !has_dot {
                has_dot = true;
                self.advance();
            } else {
                break;
            }
        }

        let num_str = &self.input[start..self.pos];

        if has_dot {
            let value: f64 = num_str
                .parse()
                .map_err(|_| PolicyParseError::InvalidNumber(num_str.to_string()))?;
            Ok(PolicyExpr::literal(PolicyLiteral::Float(value)))
        } else {
            let value: i64 = num_str
                .parse()
                .map_err(|_| PolicyParseError::InvalidNumber(num_str.to_string()))?;
            Ok(PolicyExpr::literal(PolicyLiteral::Int(value)))
        }
    }

    fn parse_identifier_or_call(&mut self) -> Result<PolicyExpr, PolicyParseError> {
        let first_ident = self.parse_identifier()?;

        self.skip_whitespace();

        // Check for qualified name (auth.*, object.*, bucket.*, etc.)
        if self.match_char('.') {
            let second_ident = self.parse_identifier()?;
            self.skip_whitespace();

            // Check if it's auth.* builtin
            if first_ident.eq_ignore_ascii_case("auth") {
                // Parse arguments
                if !self.match_char('(') {
                    return Err(PolicyParseError::UnexpectedToken(
                        "expected ( for auth builtin".to_string(),
                    ));
                }

                let args = self.parse_args()?;

                // Validate builtin
                validate_builtin_call(&second_ident, args.len())
                    .map_err(|e| PolicyParseError::InvalidBuiltin(e.to_string()))?;

                return Ok(PolicyExpr::auth_builtin(second_ident, args));
            }

            // Check for domain-specific builtins (object.key, bucket.name, etc.)
            if is_domain_builtin(&first_ident) {
                // Parse optional arguments
                let args = if self.match_char('(') {
                    self.parse_args()?
                } else {
                    vec![]
                };

                return Ok(PolicyExpr::domain_builtin(first_ident, second_ident, args));
            }

            // Check for further qualification (table.column.field)
            if self.peek() == Some('.') {
                self.advance();
                let _third_ident = self.parse_identifier()?;
                // For now, treat as table.column and ignore further qualification
                return Ok(PolicyExpr::qualified_column(first_ident, second_ident));
            }

            // Qualified column (table.column)
            return Ok(PolicyExpr::qualified_column(first_ident, second_ident));
        }

        // Check for function call
        if self.peek() == Some('(') {
            self.advance();
            let args = self.parse_args()?;
            // Generic function call - not auth builtin
            return Ok(PolicyExpr::new(PolicyExprKind::AuthBuiltin {
                name: first_ident,
                args,
            }));
        }

        // Plain column reference
        Ok(PolicyExpr::column(first_ident))
    }

    fn parse_identifier(&mut self) -> Result<String, PolicyParseError> {
        self.skip_whitespace();

        let start = self.pos;

        // First char must be letter or underscore
        match self.peek() {
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                self.advance();
            }
            _ => {
                return Err(PolicyParseError::UnexpectedToken(
                    "expected identifier".to_string(),
                ))
            }
        }

        // Rest can include digits
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }

        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_args(&mut self) -> Result<Vec<PolicyExpr>, PolicyParseError> {
        let mut args = Vec::new();

        self.skip_whitespace();

        if self.peek() == Some(')') {
            self.advance();
            return Ok(args);
        }

        loop {
            let arg = self.parse_expr()?;
            args.push(arg);

            self.skip_whitespace();

            if self.match_char(')') {
                break;
            }

            if !self.match_char(',') {
                return Err(PolicyParseError::UnexpectedToken(
                    "expected , or )".to_string(),
                ));
            }
        }

        Ok(args)
    }

    fn parse_list(&mut self) -> Result<Vec<PolicyExpr>, PolicyParseError> {
        self.skip_whitespace();

        if !self.match_char('(') {
            return Err(PolicyParseError::UnexpectedToken(
                "expected ( for IN list".to_string(),
            ));
        }

        let mut items = Vec::new();

        self.skip_whitespace();

        if self.peek() == Some(')') {
            self.advance();
            return Ok(items);
        }

        loop {
            let item = self.parse_primary()?;
            items.push(item);

            self.skip_whitespace();

            if self.match_char(')') {
                break;
            }

            if !self.match_char(',') {
                return Err(PolicyParseError::UnexpectedToken(
                    "expected , or )".to_string(),
                ));
            }
        }

        Ok(items)
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += c.len_utf8();
        }
    }

    fn match_char(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn match_str(&mut self, s: &str) -> bool {
        if self.input[self.pos..].starts_with(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn match_keyword(&mut self, keyword: &str) -> bool {
        let remaining = &self.input[self.pos..];

        if remaining.len() < keyword.len() {
            return false;
        }

        let potential = &remaining[..keyword.len()];
        if !potential.eq_ignore_ascii_case(keyword) {
            return false;
        }

        // Make sure it's not part of a longer identifier
        let next_char = remaining[keyword.len()..].chars().next();
        if let Some(c) = next_char {
            if c.is_ascii_alphanumeric() || c == '_' {
                return false;
            }
        }

        self.pos += keyword.len();
        true
    }
}

/// Check if an identifier is a known domain for domain-specific builtins.
fn is_domain_builtin(domain: &str) -> bool {
    matches!(
        domain.to_lowercase().as_str(),
        "object" | "bucket" | "row" | "table" | "connect"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_comparison() {
        let expr = parse_policy_expr("org_id = auth.org_id()").unwrap();
        match &expr.kind {
            PolicyExprKind::BinaryOp { left, op, right } => {
                assert!(matches!(&left.kind, PolicyExprKind::Column { name } if name == "org_id"));
                assert_eq!(*op, PolicyBinaryOp::Eq);
                assert!(
                    matches!(&right.kind, PolicyExprKind::AuthBuiltin { name, args } if name == "org_id" && args.is_empty())
                );
            }
            _ => panic!("expected binary op"),
        }
    }

    #[test]
    fn test_and_or() {
        let expr = parse_policy_expr("a = 1 AND b = 2 OR c = 3").unwrap();
        // Should parse as (a=1 AND b=2) OR c=3
        match &expr.kind {
            PolicyExprKind::BinaryOp { op, .. } => {
                assert_eq!(*op, PolicyBinaryOp::Or);
            }
            _ => panic!("expected OR at top level"),
        }
    }

    #[test]
    fn test_not() {
        let expr = parse_policy_expr("NOT active").unwrap();
        match &expr.kind {
            PolicyExprKind::UnaryOp {
                op: PolicyUnaryOp::Not,
                ..
            } => {}
            _ => panic!("expected NOT"),
        }
    }

    #[test]
    fn test_is_null() {
        let expr = parse_policy_expr("deleted_at IS NULL").unwrap();
        match &expr.kind {
            PolicyExprKind::IsNull { negated, .. } => {
                assert!(!negated);
            }
            _ => panic!("expected IS NULL"),
        }
    }

    #[test]
    fn test_is_not_null() {
        let expr = parse_policy_expr("created_at IS NOT NULL").unwrap();
        match &expr.kind {
            PolicyExprKind::IsNull { negated, .. } => {
                assert!(*negated);
            }
            _ => panic!("expected IS NOT NULL"),
        }
    }

    #[test]
    fn test_in_list() {
        let expr = parse_policy_expr("status IN ('active', 'pending')").unwrap();
        match &expr.kind {
            PolicyExprKind::InList { list, negated, .. } => {
                assert!(!negated);
                assert_eq!(list.len(), 2);
            }
            _ => panic!("expected IN"),
        }
    }

    #[test]
    fn test_auth_has_permission() {
        let expr = parse_policy_expr("auth.has_permission('admin')").unwrap();
        match &expr.kind {
            PolicyExprKind::AuthBuiltin { name, args } => {
                assert_eq!(name, "has_permission");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected auth builtin"),
        }
    }

    #[test]
    fn test_string_with_quotes() {
        let expr = parse_policy_expr("name = 'it''s ok'").unwrap();
        match &expr.kind {
            PolicyExprKind::BinaryOp { right, .. } => match &right.kind {
                PolicyExprKind::Literal(PolicyLiteral::String(s)) => {
                    assert_eq!(s, "it's ok");
                }
                _ => panic!("expected string literal"),
            },
            _ => panic!("expected binary op"),
        }
    }

    #[test]
    fn test_qualified_column() {
        let expr = parse_policy_expr("users.org_id = auth.org_id()").unwrap();
        match &expr.kind {
            PolicyExprKind::BinaryOp { left, .. } => match &left.kind {
                PolicyExprKind::QualifiedColumn { table, column } => {
                    assert_eq!(table, "users");
                    assert_eq!(column, "org_id");
                }
                _ => panic!("expected qualified column"),
            },
            _ => panic!("expected binary op"),
        }
    }

    #[test]
    fn test_complex_policy() {
        let input = "(org_id = auth.org_id() AND status = 'active') OR auth.has_permission('*')";
        let expr = parse_policy_expr(input).unwrap();
        // Just verify it parses without error
        assert!(matches!(expr.kind, PolicyExprKind::BinaryOp { .. }));
    }

    #[test]
    fn test_domain_builtin_object_key() {
        let expr = parse_policy_expr("object.key LIKE 'uploads/%'").unwrap();
        match &expr.kind {
            PolicyExprKind::BinaryOp { left, op, .. } => {
                assert_eq!(*op, PolicyBinaryOp::Like);
                match &left.kind {
                    PolicyExprKind::DomainBuiltin { domain, name, .. } => {
                        assert_eq!(domain, "object");
                        assert_eq!(name, "key");
                    }
                    _ => panic!("expected domain builtin"),
                }
            }
            _ => panic!("expected binary op"),
        }
    }
}
