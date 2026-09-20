//! The `grep -E` subset the harness's patterns use: literals, the `.`
//! wildcard, `[0-9]`-style classes, `(a|b)` groups, the `*` and `+`
//! quantifiers, and a leading `^` anchor. Patterns parse once and match
//! per line by backtracking with continuations — an existence verdict,
//! which is all grep's exit status ever reported to the shell harness.
//! The full ERE grammar this is not; the shapes above are the whole
//! vocabulary of the harness's assertions, kept hand-rolled per the
//! family doctrine of dependency-light binaries.

#[derive(Clone, Debug, PartialEq)]
enum Atom {
    /// A literal character; an escaped metacharacter lands here too.
    Lit(char),
    /// `.` — any one character.
    Any,
    /// `[lo-hi ...]` — one character inside the given ranges.
    Class(Vec<(char, char)>),
    /// `(a|b|c)` — any one of the alternative sequences.
    Group(Vec<Vec<Piece>>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Quant {
    One,
    Star,
    Plus,
}

#[derive(Clone, Debug, PartialEq)]
struct Piece {
    atom: Atom,
    quant: Quant,
}

/// One compiled pattern. Building is infallible because every pattern
/// the harness carries is a compile-time constant that parses.
pub struct Pattern {
    anchored: bool,
    seq: Vec<Piece>,
}

impl Pattern {
    pub fn new(pattern: &str) -> Pattern {
        let mut parser = Parser {
            chars: pattern.chars().collect(),
            pos: 0,
        };
        let anchored = parser.peek() == Some('^');
        if anchored {
            parser.pos += 1;
        }
        let seq = parser.sequence();
        Pattern { anchored, seq }
    }

    /// Does any line of `text` match? (The shell's `grep -Eq pattern
    /// file`.)
    pub fn matches_any_line(&self, text: &str) -> bool {
        text.lines().any(|line| self.is_match(line))
    }

    /// How many lines of `text` match? (The shell's `grep -cE`.)
    pub fn count_lines(&self, text: &str) -> usize {
        text.lines().filter(|line| self.is_match(line)).count()
    }

    /// Does `line` match? Unanchored patterns may start anywhere, as
    /// grep does.
    pub fn is_match(&self, line: &str) -> bool {
        let chars: Vec<char> = line.chars().collect();
        if self.anchored {
            return matches_seq(&self.seq, &chars, 0, &mut |_| true);
        }
        (0..=chars.len()).any(|start| matches_seq(&self.seq, &chars, start, &mut |_| true))
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// One sequence of pieces, stopping at end, `|`, or `)` (the
    /// terminator belongs to the caller).
    fn sequence(&mut self) -> Vec<Piece> {
        let mut pieces: Vec<Piece> = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            match c {
                '*' => {
                    let quant = pieces.last_mut().expect("`*` follows an atom");
                    quant.quant = Quant::Star;
                    self.pos += 1;
                }
                '+' => {
                    let quant = pieces.last_mut().expect("`+` follows an atom");
                    quant.quant = Quant::Plus;
                    self.pos += 1;
                }
                '.' => {
                    pieces.push(one(Atom::Any));
                    self.pos += 1;
                }
                '(' => {
                    self.pos += 1;
                    let alternatives = self.alternatives();
                    assert_eq!(self.peek(), Some(')'), "group is closed");
                    self.pos += 1;
                    pieces.push(one(Atom::Group(alternatives)));
                }
                '[' => {
                    let ranges = self.class_body();
                    pieces.push(one(Atom::Class(ranges)));
                }
                '\\' => {
                    self.pos += 1;
                    let literal = self.peek().expect("escape has a character");
                    pieces.push(one(Atom::Lit(literal)));
                    self.pos += 1;
                }
                literal => {
                    pieces.push(one(Atom::Lit(literal)));
                    self.pos += 1;
                }
            }
        }
        pieces
    }

    /// The `|`-separated sequences of a group, positioned after `(` and
    /// leaving the closing `)` for the caller.
    fn alternatives(&mut self) -> Vec<Vec<Piece>> {
        let mut alternatives = vec![self.sequence()];
        while self.peek() == Some('|') {
            self.pos += 1;
            alternatives.push(self.sequence());
        }
        alternatives
    }

    /// The ranges of a class, positioned at `[` and consuming the `]`.
    fn class_body(&mut self) -> Vec<(char, char)> {
        self.pos += 1; // the '['
        let mut ranges = Vec::new();
        while let Some(c) = self.peek() {
            if c == ']' {
                self.pos += 1;
                break;
            }
            self.pos += 1;
            let lo = c;
            let hi = if self.peek() == Some('-')
                && self.chars.get(self.pos + 1).is_some_and(|c| *c != ']')
            {
                self.pos += 2;
                self.chars[self.pos - 1]
            } else {
                lo
            };
            ranges.push((lo, hi));
        }
        ranges
    }
}

fn one(atom: Atom) -> Piece {
    Piece {
        atom,
        quant: Quant::One,
    }
}

/// Match `seq` at `i`, handing the end position to `k` — the
/// continuation threading that lets groups and repetitions backtrack.
fn matches_seq(seq: &[Piece], s: &[char], i: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    let Some(first) = seq.first() else {
        return k(i);
    };
    let rest = &seq[1..];
    match first.quant {
        Quant::One => matches_atom(&first.atom, s, i, &mut |j| matches_seq(rest, s, j, k)),
        Quant::Star => repeats(&first.atom, s, i, 0, k),
        Quant::Plus => repeats(&first.atom, s, i, 1, k),
    }
}

/// `need` further repetitions of `atom`, greedily, then `k` — the
/// greedy-first order explores the whole tree before giving up, so the
/// existence verdict keeps grep's semantics.
fn repeats(
    atom: &Atom,
    s: &[char],
    i: usize,
    need: usize,
    k: &mut dyn FnMut(usize) -> bool,
) -> bool {
    let mut greedy = |j: usize| j > i && repeats(atom, s, j, need.saturating_sub(1), k);
    if matches_atom(atom, s, i, &mut greedy) {
        return true;
    }
    need == 0 && k(i)
}

fn matches_atom(atom: &Atom, s: &[char], i: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    let hit = match atom {
        Atom::Lit(c) => i < s.len() && s[i] == *c,
        Atom::Any => i < s.len(),
        Atom::Class(ranges) => {
            i < s.len() && ranges.iter().any(|(lo, hi)| (*lo..=*hi).contains(&s[i]))
        }
        Atom::Group(alternatives) => {
            return alternatives
                .iter()
                .any(|alternative| matches_seq(alternative, s, i, k))
        }
    };
    hit && k(i + 1)
}

#[cfg(test)]
mod tests {
    use super::Pattern;

    fn p(pattern: &str) -> Pattern {
        Pattern::new(pattern)
    }

    #[test]
    fn literal_matches_anywhere() {
        assert!(p("DEMO PASSED").is_match("DEMO PASSED — all verify outcomes matched"));
        assert!(p("topology: LIVE").is_match("    topology: LIVE — four sibling services"));
        assert!(!p("topology: LIVE").is_match("topology: LOCAL"));
    }

    #[test]
    fn anchored_beat_header_counts_b_numbered_lines_only() {
        let header = p("^B[0-9]+ — ");
        assert!(header.is_match("B1 — Assembly in Kyoto"));
        assert!(header.is_match("B10 — End of life"));
        // B-CTO, B-INT, and B-QUORUM ride between the numbered beats
        // and must not count.
        assert!(!header.is_match("B-QUORUM — Retroactive distrust"));
        assert!(!header.is_match("    B4 — The border moment"));
        let transcript = "B1 — Assembly in Kyoto\n  a line\nB2 — Parts\nB-INT — S12\n";
        assert_eq!(header.count_lines(transcript), 2);
    }

    #[test]
    fn wildcard_then_alternation_covers_verdict_spellings() {
        let border = p("B4 border moment.*(PASS|== 0)");
        assert!(border.is_match("B4 border moment: verdict PASS"));
        assert!(border.is_match("B4 border moment ... check == 0"));
        assert!(!border.is_match("B4 border moment: DEGRADED"));
        let swap = p("B7 new pack .post-swap..*== 0");
        assert!(swap.is_match("B7 new pack (post-swap) verify == 0"));
        assert!(!swap.is_match("B7 new pack (post-swap) verify == 2"));
    }

    #[test]
    fn escaped_brackets_are_literals() {
        let identical = p(r"\[ok\].*issuer pack anchor == trust-pinned anchor");
        assert!(identical.is_match("  [ok]   issuer pack anchor == trust-pinned anchor"));
        assert!(!identical.is_match("  [FAIL] issuer pack anchor differs"));
    }

    #[test]
    fn plus_quantifiers_span_spaces_and_digits() {
        let receipts = p("log: +receipt [0-9]+ ");
        assert!(receipts.is_match("    log:   receipt 42 anchored"));
        assert!(receipts.is_match("log: receipt 7 "));
        // one space too few, or no trailing space, is not the idiom
        assert!(!receipts.is_match("log:receipt 7 "));
        assert!(!receipts.is_match("log: receipt 7"));
    }

    #[test]
    fn wildcard_parentheses_and_the_keyring_path() {
        let pinned = p("anchor pinned from .*/keyring");
        assert!(pinned.is_match("anchor pinned from http://127.0.0.1:8092/keyring"));
        assert!(!pinned.is_match("anchor pinned from /anchors"));
        let idempotent = p(".idempotent per subject. == matched");
        assert!(idempotent.is_match("B-INT re-ingest matches (idempotent per subject) == matched"));
        assert!(
            !idempotent.is_match("B-INT re-ingest matches (idempotent per subject) == differed")
        );
    }

    #[test]
    fn star_matches_empty_and_long_runs() {
        let trailing = p("B8 post-auction.*== 0");
        assert!(trailing.is_match("B8 post-auction == 0"));
        assert!(trailing.is_match("B8 post-auction verify: check == 0"));
        assert!(!trailing.is_match("B8 post-auction == 2"));
    }

    #[test]
    fn every_harness_pattern_compiles() {
        for pattern in [
            "^B[0-9]+ — ",
            "B4 border moment.*(PASS|== 0)",
            "B6 derestriction.*(FAIL|== 2)",
            "B7 new pack .post-swap..*== 0",
            "B8 post-auction.*== 0",
            "B9 original pack.*(FAIL|== 2)",
            "B9 Vienna bike.*unaffected",
            "B10 end-of-waste.*(DEGRADED|== 1)",
            "B10 decomposed bike.*degraded",
            "DEMO PASSED",
            "topology: LIVE",
            "registry : http",
            "issuer   : http.*server-minted packs",
            "trust    : http.*verify anchor source",
            "log      : http.*signed receipts",
            "anchor pinned from .*/keyring",
            r"\[ok\].*issuer pack anchor == trust-pinned anchor",
            "log: +receipt [0-9]+ ",
            "B-INT — S12 interop",
            "B-INT render source is the live issuer == issuer",
            "B-INT ingested identity round-trips to the source passport == ",
            ".idempotent per subject. == matched",
            "G-GRID the sealed segment verifies from the spine alone == ok",
            "G-GRID S13 offers attestation, not data == ok",
            "G-GRID substitution verifies under the verifier's own anchors == ok",
            "G-GRID the verdict is a coverage report object .verified-direct . attested. == ok",
            "B-QUORUM single regulator refused .422 — quorum attestation required. == 422",
            "B-QUORUM quorate 2-of-3 declaration accepted .201. == 201",
            "B-QUORUM the pack verdict degrades: in-window verifications no longer stand == false",
        ] {
            Pattern::new(pattern);
        }
    }
}
