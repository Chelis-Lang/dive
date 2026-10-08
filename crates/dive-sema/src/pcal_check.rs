//! The part of SANY's PlusCal translation check that can reject a module.
//!
//! SANY's parse phase runs `pcal.Validator.validate` on the root module and
//! on every module it extends (`SpecObj.loadSpec`, `validateParseUnit`). Most
//! outcomes are warnings (the algorithm or its translation changed), but two
//! reject the specification:
//!
//! - `ParseAlgorithm.uncomment` throws on an unterminated string in the
//!   algorithm region, and SANY reports `INTERNAL_ERROR` ("An unexpected
//!   problem was encountered attempting to validate ...");
//! - with an algorithm and an `END TRANSLATION` line but no
//!   `BEGIN TRANSLATION` line, `Validator` indexes line -1 and SANY aborts.
//!
//! This module reproduces exactly those two outcomes. The rest of the
//! validation (re-translating the algorithm to compare checksums) only yields
//! warnings and belongs to `dive-pluscal`.

/// `pcal.trans.findTokenPair`: the first line at or after `from` containing
/// `a`, then one or more spaces, then `b` at the first occurrence of `b`.
fn find_token_pair(lines: &[String], from: usize, a: &str, b: &str) -> Option<usize> {
    for (i, line) in lines.iter().enumerate().skip(from) {
        if let Some(col) = line.find(a) {
            let mut next = col + a.len();
            while line.as_bytes().get(next) == Some(&b' ') {
                next += 1;
            }
            if next < line.len() && line.find(b) == Some(next) {
                return Some(i);
            }
        }
    }
    None
}

/// `pcal.trans.removeTabs`: tabs to spaces, to the next multiple of 8.
fn remove_tabs(line: &str) -> String {
    let mut out = String::new();
    for c in line.chars() {
        if c == '\t' {
            let add = 8 - (out.chars().count() % 8);
            out.extend(std::iter::repeat_n(' ', add));
        } else {
            out.push(c);
        }
    }
    out
}

/// `ParseAlgorithm.uncomment`, reporting only whether it throws.
fn uncomment_fails(lines: &[String], beg_line: usize, beg_col: usize) -> bool {
    let mut depth = 0u32;
    let mut col = beg_col;
    for line in lines.iter().skip(beg_line) {
        let b: Vec<char> = line.chars().collect();
        let mut in_string = false;
        while col < b.len() {
            let c = b[col];
            let next = b.get(col + 1).copied();
            if c == '(' && !in_string && next == Some('*') {
                depth += 1;
                col += 1;
            } else if c == '*' && !in_string && next == Some(')') {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
                col += 1;
            } else if depth == 0 {
                if in_string {
                    if c == '"' {
                        in_string = false;
                    } else if c == '\\' && col + 1 < b.len() {
                        col += 1;
                    }
                } else if c == '\\' && next == Some('*') {
                    col = b.len();
                } else if c == '"' {
                    in_string = true;
                }
            }
            col += 1;
        }
        if in_string {
            return true;
        }
        col = 0;
    }
    false
}

/// Java's `BufferedReader.readLine` split.
fn java_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => out.push(std::mem::take(&mut cur)),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push(std::mem::take(&mut cur));
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Why SANY's PlusCal validation rejects the module spanning 1-based lines
/// `begin..=end` of `text`, if it does.
pub(crate) fn validation_failure(text: &str, begin: usize, end: usize) -> Option<&'static str> {
    let all = java_lines(text);
    let lines: Vec<String> = all
        .iter()
        .enumerate()
        .filter(|(i, _)| (begin..=end).contains(&(i + 1)))
        .map(|(_, l)| l.clone())
        .collect();
    if !lines
        .iter()
        .any(|l| l.find("algorithm").is_some_and(|i| i > 0))
    {
        return None;
    }
    let lines: Vec<String> = lines.iter().map(|l| remove_tabs(l)).collect();
    let mut found: Option<(usize, usize)> = None;
    for (i, line) in lines.iter().enumerate() {
        if !line.is_empty() && line.chars().all(|c| c == '=') {
            break;
        }
        if let Some(c) = line.find("--algorithm") {
            found = Some((i, c + "--algorithm".len()));
            break;
        }
        if let Some(c) = line.find("--fair") {
            found = Some((i, c + "--fair".len()));
            break;
        }
    }
    let begin_t = find_token_pair(&lines, 0, "BEGIN", "TRANSLATION");
    let end_t = find_token_pair(&lines, begin_t.map_or(0, |b| b + 1), "END", "TRANSLATION");
    if begin_t.is_none() && end_t.is_none() {
        return None;
    }
    let (al, ac) = found?;
    // Columns are counted in characters, as Java's (BMP) string indices.
    let ac = lines[al].get(..ac).map_or(ac, |s| s.chars().count());
    if uncomment_fails(&lines, al, ac) {
        return Some(
            "an unterminated string in the PlusCal algorithm region makes SANY's translation check fail",
        );
    }
    if begin_t.is_none() {
        return Some(
            "a PlusCal algorithm with `END TRANSLATION` but no `BEGIN TRANSLATION` makes SANY's translation check crash",
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unterminated_string_in_algorithm_fails() {
        let t = "---- MODULE M ----\n(* --algorithm a\nbegin x := \"abc;\nend algorithm *)\n\\* BEGIN TRANSLATION\n\\* END TRANSLATION\n====\n";
        assert!(validation_failure(t, 1, 7).is_some());
        let ok = t.replace("\"abc;", "\"abc\";");
        assert!(validation_failure(&ok, 1, 7).is_none());
        let no_xlation =
            "---- MODULE M ----\n(* --algorithm a\nbegin x := \"abc;\nend algorithm *)\n====\n";
        assert!(validation_failure(no_xlation, 1, 5).is_none());
    }

    #[test]
    fn end_without_begin_translation_fails() {
        let t = "---- MODULE M ----\n(* --algorithm a\nbegin skip;\nend algorithm *)\n\\* END TRANSLATION\n====\n";
        assert!(validation_failure(t, 1, 6).is_some());
    }
}
