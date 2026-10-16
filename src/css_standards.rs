//! Standards-tokenizer intake boundary. This does not replace CSS cascade or paint.
use cssparser::{Parser, ParserInput, Token};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CssTokenAudit {
    pub tokens: usize,
    pub functions: usize,
    pub at_keywords: usize,
    pub dimensions: usize,
    pub comments: usize,
}

/// A bounded scan of standards-tokenized CSS. Unlike string splitting, quoted
/// delimiters and CSS escapes are handled by cssparser. Nested blocks are
/// deliberately counted as tokens rather than claiming semantic evaluation.
pub fn audit_css_tokens(source: &str) -> Result<CssTokenAudit, &'static str> {
    if source.len() > 4_000_000 { return Err("CSS source size limit exceeded"); }
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut audit = CssTokenAudit::default();
    loop {
        let next = parser.next_including_whitespace_and_comments();
        let token = match next {
            Ok(token) => token.clone(),
            Err(_) => break,
        };
        audit.tokens += 1;
        if audit.tokens > 1_000_000 { return Err("CSS token count limit exceeded"); }
        match token {
            Token::Function(_) => audit.functions += 1,
            Token::AtKeyword(_) => audit.at_keywords += 1,
            Token::Dimension { .. } => audit.dimensions += 1,
            Token::Comment(_) => audit.comments += 1,
            _ => {}
        }
    }
    Ok(audit)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_css_tokenizer_handles_escaped_delimiters_and_comments() {
        let source = r#"/* entry */ @media screen { .x::before { content: "a;b:c"; width: calc(100% - 2rem) } }"#;
        let audit = audit_css_tokens(source).unwrap();
        assert!(audit.comments >= 1);
        assert!(audit.at_keywords >= 1);
        assert!(audit.tokens >= 7);
    }
    #[test]
    fn resource_limit_is_enforced_before_parsing() {
        assert_eq!(audit_css_tokens(&"x".repeat(4_000_001)),
            Err("CSS source size limit exceeded"));
    }
}
