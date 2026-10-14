//! Native canonical CSS declaration syntax, before capability lowering.
//! This representation preserves source ordering and nested expression boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssSyntaxError {
    UnclosedString,
    UnbalancedDelimiter,
    InvalidDeclaration,
    LimitExceeded,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalDeclaration {
    pub property: String,
    /// Exact source-preserving value retained even when semantic lowering is unavailable.
    pub value: String,
    pub value_tree: CanonicalValue,
    pub important: bool,
    pub order: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalValue {
    Components(Vec<CssComponentValue>),
    UnsupportedRaw(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalDeclarationList {
    pub declarations: Vec<CanonicalDeclaration>,
}
fn scan_parts(input: &str, separator: char) -> Result<Vec<&str>, CssSyntaxError> {
    if input.len() > 4_000_000 { return Err(CssSyntaxError::LimitExceeded); }
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quotes = None;
    let mut escaped = false;
    let mut depth = Vec::new();
    for (offset, ch) in input.char_indices() {
        if let Some(q) = quotes {
            if escaped { escaped = false; continue; }
            if ch == '\\' { escaped = true; continue; }
            if ch == q { quotes = None; }
            continue;
        }
        match ch {
            '"' | '\'' => quotes = Some(ch),
            '(' | '[' | '{' => { depth.push(ch); if depth.len() > 128 { return Err(CssSyntaxError::LimitExceeded); } },
            ')' | ']' | '}' => {
                let valid = matches!((depth.pop(),ch),(Some('('),')')|(Some('['),']')|(Some('{'),'}'));
                if !valid { return Err(CssSyntaxError::UnbalancedDelimiter); }
            },
            _ if ch == separator && depth.is_empty() => {
                parts.push(&input[start..offset]); start = offset + ch.len_utf8();
                if parts.len() > 100_000 { return Err(CssSyntaxError::LimitExceeded); }
            }
            _ => {}
        }
    }
    if quotes.is_some() { return Err(CssSyntaxError::UnclosedString); }
    if !depth.is_empty() { return Err(CssSyntaxError::UnbalancedDelimiter); }
    parts.push(&input[start..]);
    Ok(parts)
}
fn split_important(raw: &str) -> (&str, bool) {
    let s = raw.trim();
    let mut quote = None;
    let mut escape = false;
    let mut depth = 0usize;
    let mut marker = None;
    for (i, c) in s.char_indices() {
        if let Some(q) = quote {
            if escape { escape = false; continue; }
            if c == '\\' { escape = true; continue; }
            if c == q { quote = None; }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            '!' if depth == 0 => marker = Some(i),
            _ => {}
        }
    }
    if let Some(i) = marker {
        if s[i + 1..].trim().eq_ignore_ascii_case("important") {
            return (s[..i].trim_end(), true);
        }
    }
    (s, false)
}
pub fn parse_declaration_list(input: &str) -> Result<CanonicalDeclarationList, CssSyntaxError> {
    let mut result = Vec::new();
    for part in scan_parts(input,';')? {
        let part = part.trim();
        if part.is_empty() { continue; }
        let pieces = scan_parts(part, ':')?;
        if pieces.len() < 2 { return Err(CssSyntaxError::InvalidDeclaration); }
        let name = pieces[0].trim();
        if name.is_empty() || name.chars().any(char::is_whitespace) {
            return Err(CssSyntaxError::InvalidDeclaration);
        }
        let source = part[pieces[0].len()+1..].trim();
        let (value, important) = split_important(source);
        if value.is_empty() { return Err(CssSyntaxError::InvalidDeclaration); }
        result.push(CanonicalDeclaration {
            property: if name.starts_with("--") { name.into() } else { name.to_ascii_lowercase() },
            value: value.into(),
            value_tree: match tokenize_value(value) {
                Ok(tokens) => CanonicalValue::Components(tokens),
                Err(_) => CanonicalValue::UnsupportedRaw(value.into()),
            },
            important, order: result.len(),
        });
    }
    Ok(CanonicalDeclarationList { declarations: result })
}
/// Token-level canonical value representation. It does not infer CSS semantics;
/// unknown functions and units remain represented rather than being silently dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssComponentValue {
    Ident(String),
    Number(String),
    Dimension { number: String, unit: String },
    StringLiteral(String),
    Function { name: String, arguments: Vec<CssComponentValue> },
    Delimiter(char),
}

pub fn tokenize_value(input: &str) -> Result<Vec<CssComponentValue>, CssSyntaxError> {
    if input.len() > 1_000_000 { return Err(CssSyntaxError::LimitExceeded); }
    fn parse(input: &str, depth: usize) -> Result<Vec<CssComponentValue>, CssSyntaxError> {
        if depth > 64 { return Err(CssSyntaxError::LimitExceeded); }
        let chars = input.chars().collect::<Vec<_>>();
        let mut result = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i].is_whitespace() { i += 1; continue; }
            if chars[i] == '"' || chars[i] == '\'' {
                let quote = chars[i]; i += 1;
                let mut value = String::new();
                let mut closed = false;
                while i < chars.len() {
                    if chars[i] == '\\' && i+1 < chars.len() {
                        value.push(chars[i]); value.push(chars[i+1]); i+=2; continue;
                    }
                    if chars[i] == quote { i+=1;closed=true;break; }
                    value.push(chars[i]);i+=1;
                }
                if !closed { return Err(CssSyntaxError::UnclosedString); }
                result.push(CssComponentValue::StringLiteral(value));
                continue;
            }
            if chars[i].is_ascii_digit() || (chars[i]=='.' && i+1<chars.len() && chars[i+1].is_ascii_digit()) {
                let start=i;
                while i<chars.len() && (chars[i].is_ascii_digit() || chars[i]=='.') { i+=1; }
                let number=chars[start..i].iter().collect::<String>();
                let start_unit=i;
                while i<chars.len() && (chars[i].is_ascii_alphabetic() || chars[i]=='%') { i+=1; }
                if start_unit<i {
                    result.push(CssComponentValue::Dimension{ number, unit:chars[start_unit..i].iter().collect() });
                } else { result.push(CssComponentValue::Number(number)); }
                continue;
            }
            if chars[i].is_alphabetic() || chars[i]=='-' || chars[i]=='_' {
                let start=i;
                while i<chars.len() && (chars[i].is_alphanumeric() || matches!(chars[i],'-'|'_')) { i+=1; }
                let name=chars[start..i].iter().collect::<String>();
                if i<chars.len() && chars[i]=='(' {
                    i+=1; let content_start=i; let mut level=1; let mut quote=None;let mut escaped=false;
                    while i<chars.len() && level>0 {
                        let c=chars[i];
                        if let Some(q)=quote {
                            if escaped {escaped=false}
                            else if c=='\\' {escaped=true}
                            else if c==q {quote=None}
                        } else {
                            match c { '"'|'\''=>quote=Some(c),'('=>level+=1,')'=>level-=1,_=>{} }
                        }
                        i+=1;
                    }
                    if level!=0 { return Err(CssSyntaxError::UnbalancedDelimiter); }
                    let body=chars[content_start..i-1].iter().collect::<String>();
                    result.push(CssComponentValue::Function{name,arguments:parse(&body,depth+1)?});
                } else {result.push(CssComponentValue::Ident(name));}
                continue;
            }
            if matches!(chars[i],'(' | ')' | '[' | ']' | '{' | '}') {
                return Err(CssSyntaxError::UnbalancedDelimiter);
            }
            result.push(CssComponentValue::Delimiter(chars[i])); i+=1;
            if result.len() > 100_000 { return Err(CssSyntaxError::LimitExceeded); }
        }
        Ok(result)
    }
    parse(input,0)
}

/// Source-preserving stylesheet rule tree. A block is retained without claiming
/// its contents are supported by the layout engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalRule {
    Style { selector: String, declarations: CanonicalDeclarationList },
    AtRule { name: String, prelude: String, body: Option<CanonicalBlock> },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalBlock {
    Rules(Vec<CanonicalRule>),
    Declarations(CanonicalDeclarationList),
    UnsupportedRaw(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalStylesheet {
    pub rules: Vec<CanonicalRule>,
}
pub fn parse_stylesheet_structure(input: &str) -> Result<CanonicalStylesheet, CssSyntaxError> {
    parse_stylesheet_structure_inner(input, 0)
}
fn parse_stylesheet_structure_inner(input: &str, depth: usize) -> Result<CanonicalStylesheet, CssSyntaxError> {
    if depth >= 64 || input.len() > 8_000_000 {return Err(CssSyntaxError::LimitExceeded);}
    let mut i=0;
    let bytes=input.as_bytes();
    let mut rules=Vec::new();
    while i < bytes.len() {
        while i<bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i]==b';') {i+=1;}
        if i>=bytes.len(){break;}
        if i+1<bytes.len() && &bytes[i..i+2]==b"/*" {
            let tail=input[i+2..].find("*/").ok_or(CssSyntaxError::UnbalancedDelimiter)?;
            i+=tail+4;continue;
        }
        let start=i;
        let mut quotes=None;
        let mut escaped=false;
        let mut paren=0usize;
        let mut square=0usize;
        let mut end=None;
        while i<bytes.len() {
            let c=bytes[i];
            if let Some(q)=quotes {
                if escaped {escaped=false;}
                else if c==b'\\' {escaped=true;}
                else if c==q {quotes=None;}
                i+=1;continue;
            }
            match c {
                b'"' | b'\'' => quotes=Some(c),
                b'(' => paren+=1,
                b')' => {if paren==0 {return Err(CssSyntaxError::UnbalancedDelimiter);} paren-=1;},
                b'[' => square+=1,
                b']' => {if square==0 {return Err(CssSyntaxError::UnbalancedDelimiter);} square-=1;},
                b'{' | b';' if paren==0 && square==0 => {end=Some(c);break;},
                b'}' if paren==0 && square==0 => return Err(CssSyntaxError::UnbalancedDelimiter),
                _=>{}
            }
            i+=1;
        }
        let delimiter=end.ok_or(CssSyntaxError::UnbalancedDelimiter)?;
        let prelude=input[start..i].trim();
        if prelude.is_empty() {return Err(CssSyntaxError::InvalidDeclaration);}
        if delimiter==b';' {
            if !prelude.starts_with('@'){return Err(CssSyntaxError::InvalidDeclaration);}
            let inner=&prelude[1..];
            let cut=inner.find(char::is_whitespace).unwrap_or(inner.len());
            rules.push(CanonicalRule::AtRule {name:inner[..cut].to_ascii_lowercase(),prelude:inner[cut..].trim().into(),body:None});
            i+=1;continue;
        }
        i+=1;
        let body_start=i;
        let mut level=1usize;
        let mut quotes=None;
        let mut escaped=false;
        let mut comment=false;
        while i<bytes.len() && level>0 {
            if comment {
                if i+1<bytes.len() && &bytes[i..i+2]==b"*/" {comment=false;i+=2;continue;}
                i+=1;continue;
            }
            if let Some(q)=quotes {
                if escaped {escaped=false;}
                else if bytes[i]==b'\\' {escaped=true;}
                else if bytes[i]==q {quotes=None;}
                i+=1;continue;
            }
            if i+1<bytes.len() && &bytes[i..i+2]==b"/*" {comment=true;i+=2;continue;}
            match bytes[i] {
                b'"' | b'\'' => quotes=Some(bytes[i]),
                b'{' => level+=1,
                b'}' => level-=1,
                _=>{}
            }
            if level>128 {return Err(CssSyntaxError::LimitExceeded);}
            i+=1;
        }
        if level!=0 {return Err(CssSyntaxError::UnbalancedDelimiter);}
        let body=&input[body_start..i-1];
        if let Some(rest)=prelude.strip_prefix('@') {
            let cut=rest.find(char::is_whitespace).unwrap_or(rest.len());
            let name = rest[..cut].to_ascii_lowercase();
            let block = match name.as_str() {
                "media" | "supports" | "container" | "layer" | "keyframes" => {
                    CanonicalBlock::Rules(parse_stylesheet_structure_inner(body, depth+1)?.rules)
                }
                "font-face" | "property" | "page" => {
                    CanonicalBlock::Declarations(parse_declaration_list(body)?)
                }
                _ => CanonicalBlock::UnsupportedRaw(body.into()),
            };
            rules.push(CanonicalRule::AtRule {
                name,
                prelude:rest[cut..].trim().into(),
                body:Some(block),
            });
        } else {
            rules.push(CanonicalRule::Style {
                selector:prelude.into(),
                declarations:parse_declaration_list(body)?,
            });
        }
        if rules.len()>100_000 {return Err(CssSyntaxError::LimitExceeded);}
    }
    Ok(CanonicalStylesheet{rules})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_stylesheet_preserves_at_rules_and_nested_css() {
        let parsed = parse_stylesheet_structure("@charset \"utf-8\"; @media (min-width: 600px) { .x { color: red; } } p { content: \"a;b:c\"; }").unwrap();
        assert_eq!(parsed.rules.len(),3);
        assert!(matches!(&parsed.rules[1],CanonicalRule::AtRule{name,body:Some(_),..} if name=="media"));
        assert!(matches!(&parsed.rules[2],CanonicalRule::Style{declarations,..} if declarations.declarations[0].value=="\"a;b:c\""));
        assert!(parse_stylesheet_structure("@media screen { p { color: red }").is_err());
    }
    #[test]
    fn canonical_value_tree_preserves_unknown_functions_and_dimensions() {
        let tokens=tokenize_value("calc(2.25rem + var(--spacing, 2px))").unwrap();
        assert_eq!(tokens.len(),1);
        match &tokens[0] {
            CssComponentValue::Function{name,arguments} => {
                assert_eq!(name,"calc");
                assert!(matches!(arguments[0],CssComponentValue::Dimension{ref number,ref unit} if number=="2.25" && unit=="rem"));
                assert!(arguments.iter().any(|item|matches!(item,CssComponentValue::Function{name,..} if name=="var")));
            }
            _=>panic!("calculation structure discarded")
        }
        assert!(tokenize_value("calc(2px").is_err());
    }
    #[test]
    fn nested_syntax_preserves_semicolons_colons_and_cascade() {
        let d=parse_declaration_list("content: \"a:b;c\"; --Brand: url(\"data:image/svg+xml;a:b\"); margin-top: var(--x, calc(2em + 1px)) !important;").unwrap();
        assert_eq!(d.declarations.len(),3);
        assert_eq!(d.declarations[0].value,"\"a:b;c\"");
        assert_eq!(d.declarations[1].property,"--Brand");
        assert_eq!(d.declarations[2].value,"var(--x, calc(2em + 1px))");
        assert!(d.declarations[2].important);
    }
    #[test]
    fn malformed_or_excessively_nested_rejects_without_guessing() {
        for s in ["color: \"unterminated", "x: calc(1px", "x: 1px)", "missing", "x: a]"] {
            assert!(parse_declaration_list(s).is_err(),"{s}");
        }
        assert_eq!(parse_declaration_list("color: red;").unwrap().declarations[0].order,0);
    }
}
