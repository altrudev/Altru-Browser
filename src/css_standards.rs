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


/// Bounded RGB palette conversion at the native paint boundary.
pub fn parse_native_color(input: &str) -> Option<[u8;4]> {
    let value = input.trim().to_ascii_lowercase();
    if value == "transparent" { return Some([0,0,0,0]); }
    let hex = value.strip_prefix('#');
    if let Some(h) = hex {
        let expanded = if h.len() == 3 || h.len() == 4 {
            h.chars().flat_map(|c| [c,c]).collect::<String>()
        } else { h.to_owned() };
        if expanded.len() != 6 && expanded.len() != 8 { return None; }
        let bytes = expanded.as_bytes().chunks(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?,16).ok())
            .collect::<Option<Vec<_>>>()?;
        return Some([bytes[0],bytes[1],bytes[2],*bytes.get(3).unwrap_or(&255)]);
    }
    let rgb = value.strip_prefix("rgb(").and_then(|v|v.strip_suffix(')'));
    if let Some(v) = rgb {
        let channels = v.split(',').map(|v|v.trim().parse::<u8>().ok())
            .collect::<Option<Vec<_>>>()?;
        if channels.len() == 3 { return Some([channels[0], channels[1], channels[2],255]); }
    }
    let channels = match value.as_str() {
        "black" => [0,0,0,255], "white" => [255,255,255,255],
        "red" => [255,0,0,255], "blue" => [0,0,255,255],
        "green" => [0,128,0,255], "gray" | "grey" => [128,128,128,255],
        _ => return None,
    };
    Some(channels)
}
