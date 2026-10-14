//! Bounded CSS vendor/selector classification. No silent base-selector fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendorFamily { Standard, WebKit, Gecko, Microsoft, Opera, Unknown }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorDisposition { NativeCandidate, InertWebKitPseudo, Unsupported }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectorVerdict {
    pub family: VendorFamily,
    pub disposition: SelectorDisposition,
}
pub const MAX_SELECTOR_BYTES: usize = 4096;
pub fn classify_selector(source: &str) -> SelectorVerdict {
    use SelectorDisposition::*;
    use VendorFamily::*;
    let trimmed = source.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_SELECTOR_BYTES {
        return SelectorVerdict { family: Unknown, disposition: Unsupported };
    }
    let lower = trimmed.to_ascii_lowercase();
    let prefixed = lower.rfind("::").map(|at| (&lower[..at], &lower[at+2..]));
    if let Some((base, pseudo)) = prefixed {
        for (prefix, family) in [
            ("-webkit-", WebKit), ("-moz-", Gecko),
            ("-ms-", Microsoft), ("-o-", Opera),
        ] {
            if pseudo.starts_with(prefix) {
                let tail = &pseudo[prefix.len()..];
                let syntactic_name = !tail.is_empty()
                    && tail.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
                return SelectorVerdict {
                    family,
                    disposition: if family == WebKit && (base.trim().is_empty() || !base.trim().ends_with(':')) && syntactic_name {
                        InertWebKitPseudo
                    } else { Unsupported },
                };
            }
        }
    }
    // Vendor-prefixed pseudo-classes are never covered by the pseudo-element exception.
    for (prefix, family) in [
        (":-webkit-", WebKit), (":-moz-", Gecko),
        (":-ms-", Microsoft), (":-o-", Opera),
    ] {
        if lower.contains(prefix) {
            return SelectorVerdict { family, disposition: Unsupported };
        }
    }
    SelectorVerdict { family: Standard, disposition: NativeCandidate }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn families_and_dispositions_are_distinct() {
        use SelectorDisposition::*;
        use VendorFamily::*;
        for value in [
            "::-webkit-scrollbar", ".x::-webkit-scrollbar-thumb",
            "input::-WEBKIT-file-upload-button", ".x::-webkit-future-part",
        ] {
            assert_eq!(classify_selector(value), SelectorVerdict { family: WebKit, disposition: InertWebKitPseudo });
        }
        for (value, family) in [
            (".x::-moz-range-thumb", Gecko), (".x::-ms-clear", Microsoft),
            (".x::-o-part", Opera), (".x:-webkit-autofill", WebKit),
            (".x::-webkit-functional()", WebKit),
        ] {
            assert_eq!(classify_selector(value), SelectorVerdict { family, disposition: Unsupported });
        }
        for value in ["*", ".card", "::before", ":is(.a,.b)", ".x:has(>*:nth-child(4):last-child)"] {
            assert_eq!(classify_selector(value).disposition, NativeCandidate);
        }
        assert_eq!(classify_selector(&"a".repeat(4097)).disposition, Unsupported);
    }
}
