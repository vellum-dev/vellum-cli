use std::cmp::Ordering;

// Mirrors apk-tools src/version.c so vellum ranks versions the same way apk does:
// digit{.digit}...{letter}{_suffix{#}}...{~hash}{-r#}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TokenKind {
    InitialDigit,
    Digit,
    Letter,
    Suffix,
    SuffixNumber,
    CommitHash,
    RevisionNumber,
    End,
    Invalid,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum VersionSuffix {
    Alpha,
    Beta,
    Pre,
    Rc,
    Release,
    Cvs,
    Svn,
    Git,
    Hg,
    P,
}

impl VersionSuffix {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "alpha" => Some(Self::Alpha),
            "beta" => Some(Self::Beta),
            "pre" => Some(Self::Pre),
            "rc" => Some(Self::Rc),
            "cvs" => Some(Self::Cvs),
            "svn" => Some(Self::Svn),
            "git" => Some(Self::Git),
            "hg" => Some(Self::Hg),
            "p" => Some(Self::P),
            _ => None,
        }
    }
}

struct Token<'a> {
    kind: TokenKind,
    text: &'a str,
    number: u64,
    suffix: VersionSuffix,
}

struct Tokenizer<'a> {
    remaining: &'a str,
    token: Token<'a>,
}

impl<'a> Tokenizer<'a> {
    fn new(version: &'a str) -> Self {
        let mut tokenizer = Self {
            remaining: version,
            token: Token {
                kind: TokenKind::InitialDigit,
                text: "",
                number: 0,
                suffix: VersionSuffix::Release,
            },
        };
        tokenizer.take_number();
        tokenizer
    }

    fn take_while(&mut self, accept: impl Fn(u8) -> bool) -> &'a str {
        let length = self.remaining.bytes().take_while(|&b| accept(b)).count();
        let (taken, rest) = self.remaining.split_at(length);
        self.remaining = rest;
        taken
    }

    fn take_number(&mut self) {
        let digits = self.take_while(|b| b.is_ascii_digit());
        self.token.text = digits;
        self.token.number = digits
            .bytes()
            .fold(0u64, |n, d| n.wrapping_mul(10).wrapping_add(u64::from(d - b'0')));
        if digits.is_empty() {
            self.token.kind = TokenKind::Invalid;
        }
    }

    fn advance(&mut self) {
        let Some(&next_byte) = self.remaining.as_bytes().first() else {
            self.token.kind = TokenKind::End;
            return;
        };
        let previous = self.token.kind;

        match next_byte {
            b'a'..=b'z' if previous <= TokenKind::Digit => {
                self.token.text = &self.remaining[..1];
                self.token.kind = TokenKind::Letter;
                self.remaining = &self.remaining[1..];
            }
            b'.' | b'0'..=b'9' => {
                if next_byte == b'.' {
                    if previous > TokenKind::Digit {
                        self.token.kind = TokenKind::Invalid;
                        return;
                    }
                    self.remaining = &self.remaining[1..];
                }
                self.token.kind = match previous {
                    TokenKind::InitialDigit | TokenKind::Digit => TokenKind::Digit,
                    TokenKind::Suffix => TokenKind::SuffixNumber,
                    _ => {
                        self.token.kind = TokenKind::Invalid;
                        return;
                    }
                };
                self.take_number();
            }
            b'_' if previous <= TokenKind::SuffixNumber => {
                self.remaining = &self.remaining[1..];
                let name = self.take_while(|b| b.is_ascii_lowercase());
                self.token.text = name;
                match VersionSuffix::parse(name) {
                    Some(suffix) => {
                        self.token.suffix = suffix;
                        self.token.kind = TokenKind::Suffix;
                    }
                    None => self.token.kind = TokenKind::Invalid,
                }
            }
            b'~' if previous < TokenKind::CommitHash => {
                self.remaining = &self.remaining[1..];
                let hash = self.take_while(|b| b.is_ascii_hexdigit());
                self.token.text = hash;
                self.token.kind = if hash.is_empty() {
                    TokenKind::Invalid
                } else {
                    TokenKind::CommitHash
                };
            }
            b'-' if previous < TokenKind::RevisionNumber && self.remaining.starts_with("-r") => {
                self.remaining = &self.remaining[2..];
                self.token.kind = TokenKind::RevisionNumber;
                self.take_number();
            }
            _ => self.token.kind = TokenKind::Invalid,
        }
    }
}

fn compare_tokens(a: &Token, b: &Token) -> Ordering {
    match a.kind {
        TokenKind::Digit if a.text.starts_with('0') || b.text.starts_with('0') => a.text.cmp(b.text),
        TokenKind::InitialDigit
        | TokenKind::Digit
        | TokenKind::SuffixNumber
        | TokenKind::RevisionNumber => a.number.cmp(&b.number),
        TokenKind::Letter => a.text.cmp(b.text),
        TokenKind::Suffix => a.suffix.cmp(&b.suffix),
        _ => a.text.cmp(b.text),
    }
}

pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let mut a_tokens = Tokenizer::new(a);
    let mut b_tokens = Tokenizer::new(b);

    while a_tokens.token.kind == b_tokens.token.kind && a_tokens.token.kind < TokenKind::End {
        let order = compare_tokens(&a_tokens.token, &b_tokens.token);
        if order != Ordering::Equal {
            return order;
        }
        a_tokens.advance();
        b_tokens.advance();
    }

    let (a_token, b_token) = (&a_tokens.token, &b_tokens.token);
    if a_token.kind == b_token.kind {
        return Ordering::Equal;
    }
    if a_token.kind == TokenKind::Suffix && a_token.suffix < VersionSuffix::Release {
        return Ordering::Less;
    }
    if b_token.kind == TokenKind::Suffix && b_token.suffix < VersionSuffix::Release {
        return Ordering::Greater;
    }
    b_token.kind.cmp(&a_token.kind)
}

pub fn version_gte(a: &str, b: &str) -> bool {
    compare_versions(a, b) != Ordering::Less
}

pub fn version_lt(a: &str, b: &str) -> bool {
    compare_versions(a, b) == Ordering::Less
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_equal_versions() {
        assert_eq!(compare_versions("3.10.0.0", "3.10.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0", "1.0"), Ordering::Equal);
        assert_eq!(compare_versions("0", "0"), Ordering::Equal);
    }

    #[test]
    fn compare_greater_versions() {
        assert_eq!(compare_versions("3.10.0.0", "3.9.0.0"), Ordering::Greater);
        assert_eq!(compare_versions("2.0", "1.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.10", "1.9"), Ordering::Greater);
    }

    #[test]
    fn compare_lesser_versions() {
        assert_eq!(compare_versions("3.9.0.0", "3.10.0.0"), Ordering::Less);
        assert_eq!(compare_versions("1.9", "2.0"), Ordering::Less);
        assert_eq!(compare_versions("1.9", "1.10"), Ordering::Less);
    }

    #[test]
    fn compare_different_lengths() {
        assert_eq!(compare_versions("3", "3.0"), Ordering::Less);
        assert_eq!(compare_versions("3.0", "3"), Ordering::Greater);
        assert_eq!(compare_versions("3.0.0", "3.0"), Ordering::Greater);
        assert_eq!(compare_versions("3.0", "3.0.0"), Ordering::Less);
    }

    #[test]
    fn compare_with_revision() {
        assert_eq!(compare_versions("3.10.0-r1", "3.10.0"), Ordering::Greater);
        assert_eq!(compare_versions("3.10.0-r2", "3.10.0-r1"), Ordering::Greater);
        assert_eq!(compare_versions("3.10.0-r10", "3.10.0-r9"), Ordering::Greater);
        assert_eq!(compare_versions("3.10.0-r1", "3.9.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.2-r0", "1.0.2-r0"), Ordering::Equal);
    }

    #[test]
    fn compare_with_post_release_suffix() {
        assert_eq!(
            compare_versions("3.2_git20260727-r77655", "3.1.2-r2"),
            Ordering::Greater
        );
        assert_eq!(compare_versions("3.2_git20260727", "3.2"), Ordering::Greater);
        assert_eq!(compare_versions("3.2_git20260801", "3.2_git20260727"), Ordering::Greater);
        assert_eq!(compare_versions("1.0_p1", "1.0"), Ordering::Greater);
    }

    #[test]
    fn compare_with_pre_release_suffix() {
        assert_eq!(compare_versions("0.0.11_pre4", "0.0.11"), Ordering::Less);
        assert_eq!(compare_versions("0.0.11_pre4", "0.0.10"), Ordering::Greater);
        assert_eq!(compare_versions("0.0.11_pre4", "0.0.11_pre10"), Ordering::Less);
        assert_eq!(compare_versions("1.0_alpha", "1.0_beta"), Ordering::Less);
        assert_eq!(compare_versions("1.0_rc1", "1.0_pre1"), Ordering::Greater);
    }

    #[test]
    fn compare_with_letter() {
        assert_eq!(compare_versions("1.0b", "1.0a"), Ordering::Greater);
        assert_eq!(compare_versions("1.0a", "1.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.0a", "1.0.1"), Ordering::Less);
    }

    #[test]
    fn compare_with_leading_zero_as_string() {
        assert_eq!(compare_versions("1.05", "1.1"), Ordering::Less);
        assert_eq!(compare_versions("1.010", "1.01"), Ordering::Greater);
    }

    #[test]
    fn compare_with_commit_hash() {
        assert_eq!(compare_versions("1.0~abc123-r1", "1.0~abc123-r0"), Ordering::Greater);
        assert_eq!(compare_versions("1.0~abc", "1.0"), Ordering::Greater);
    }

    #[test]
    fn compare_empty_strings() {
        assert_eq!(compare_versions("", ""), Ordering::Equal);
        assert_eq!(compare_versions("1", ""), Ordering::Greater);
        assert_eq!(compare_versions("", "1"), Ordering::Less);
    }

    #[test]
    fn compare_single_component() {
        assert_eq!(compare_versions("1", "2"), Ordering::Less);
        assert_eq!(compare_versions("2", "1"), Ordering::Greater);
        assert_eq!(compare_versions("10", "9"), Ordering::Greater);
    }

    #[test]
    fn version_gte_returns_true_when_greater_or_equal() {
        assert!(version_gte("3.10.0.0", "3.10.0.0"));
        assert!(version_gte("3.10.0.0", "3.9.0.0"));
        assert!(version_gte("4.0", "3.99"));
    }

    #[test]
    fn version_gte_returns_false_when_less() {
        assert!(!version_gte("3.9.0.0", "3.10.0.0"));
        assert!(!version_gte("2.0", "3.0"));
    }

    #[test]
    fn version_lt_returns_true_when_less() {
        assert!(version_lt("3.9.0.0", "3.10.0.0"));
        assert!(version_lt("2.0", "3.0"));
    }

    #[test]
    fn version_lt_returns_false_when_greater_or_equal() {
        assert!(!version_lt("3.10.0.0", "3.10.0.0"));
        assert!(!version_lt("3.10.0.0", "3.9.0.0"));
    }
}
