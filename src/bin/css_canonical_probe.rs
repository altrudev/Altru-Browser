//! Offline CSS syntax-inventory probe. No network access, no renderer promotion.
use std::io::{self, Read};
use adaptive_web_engine_fabric::css_canonical::{parse_stylesheet_structure, CanonicalRule, CanonicalBlock};
fn count_rules(rules: &[CanonicalRule], counts: &mut [usize;4]) {
    for r in rules {
        match r {
            CanonicalRule::Style { declarations, .. } => {counts[0]+=1;counts[1]+=declarations.declarations.len();}
            CanonicalRule::AtRule { body, .. } => {
                counts[2]+=1;
                match body {
                    Some(CanonicalBlock::Rules(inner)) => count_rules(inner,counts),
                    Some(CanonicalBlock::Declarations(d)) => counts[1]+=d.declarations.len(),
                    Some(CanonicalBlock::UnsupportedRaw(_)) => counts[3]+=1,
                    None=>{}
                }
            }
        }
    }
}
fn main() {
    let mut source = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut source) {eprintln!("READ_ERROR: {e}");std::process::exit(2);}
    match parse_stylesheet_structure(&source) {
        Ok(stylesheet) => {
            let mut counts = [0;4];
            count_rules(&stylesheet.rules,&mut counts);
            println!("CANONICAL_CSS_SYNTAX_PASS rules={} declarations={} at_rules={} preserved_unsupported_blocks={}",counts[0],counts[1],counts[2],counts[3]);
        }
        Err(e) => {eprintln!("CANONICAL_CSS_SYNTAX_BLOCK: {e:?}");std::process::exit(1);}
    }
}
