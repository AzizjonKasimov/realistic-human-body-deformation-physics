//! A custom play as written on the command line, with blanks for a grid to
//! fill (`grid.rs`): `{NAME}` takes the value given for NAME, and a sum of
//! names and numbers such as `{60-lead}` is worked out, so a gesture can
//! move one step against another while the steps between them keep their
//! total.

use super::*;
use std::collections::BTreeSet;

/// Whether `text` is a name: a letter or `_`, then letters, digits, and `_`.
pub(crate) fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A custom play as written on the command line (`--strike` or
/// `--gesture`). It may leave blanks for a grid to fill: `{NAME}`, or a sum
/// of names and numbers such as `{60-lead}`.
#[derive(Clone, Debug)]
pub(crate) enum Written {
    Strike(String),
    Gesture(String),
}

impl Written {
    /// The play as written, its blanks unfilled.
    pub(crate) fn spec(&self) -> &str {
        match self {
            Written::Strike(spec) | Written::Gesture(spec) => spec,
        }
    }

    /// The play, its blanks filled from `values` (each a name and its
    /// value). With no values, a play that leaves a blank says to give its
    /// values with `--grid`.
    pub(crate) fn play(&self, values: &[(&str, &str)]) -> Result<Play, String> {
        let spec = fill(self.spec(), values)?;
        Ok(match self {
            Written::Strike(_) => Play::Swing(Strike::parse(&spec)?, None),
            Written::Gesture(_) => Play::Gesture(Gesture::parse(&spec)?),
        })
    }

    /// The names its blanks use.
    pub(crate) fn blanks(&self) -> Result<BTreeSet<String>, String> {
        let mut names = BTreeSet::new();
        for inside in blanks_in(self.spec())? {
            for token in tokens(inside)? {
                if let Token::Name(name) = token {
                    names.insert(name);
                }
            }
        }
        Ok(names)
    }
}

/// What is written inside each blank of `spec`, in order.
fn blanks_in(spec: &str) -> Result<Vec<&str>, String> {
    let mut found = Vec::new();
    let mut rest = spec;
    while let Some(at) = rest.find(['{', '}']) {
        if rest[at..].starts_with('}') {
            return Err(format!("a `}}` closes no blank in `{spec}`"));
        }
        let after = &rest[at + 1..];
        let close = after
            .find('}')
            .ok_or_else(|| format!("a blank is left open in `{spec}`"))?;
        found.push(&after[..close]);
        rest = &after[close + 1..];
    }
    Ok(found)
}

/// `spec` with its blanks filled from `values`: `{NAME}` with NAME's value
/// as given, and a sum worked out (`{60-lead}`), written whole when it is.
fn fill(spec: &str, values: &[(&str, &str)]) -> Result<String, String> {
    let value = |name: &str| {
        values
            .iter()
            .find(|(given, _)| *given == name)
            .map(|(_, value)| *value)
            .ok_or_else(|| {
                format!("the play leaves {{{name}}} blank: give its values with --grid {name}=V1,V2,...")
            })
    };
    let mut filled = String::new();
    let mut rest = spec;
    for inside in blanks_in(spec)? {
        let at = rest.find('{').expect("each blank opens with a brace");
        filled.push_str(&rest[..at]);
        let blank = inside.trim();
        if is_name(blank) {
            filled.push_str(value(blank)?);
        } else {
            let sum = work_out(blank, &|name| {
                let text = value(name)?;
                text.parse::<f64>()
                    .map_err(|_| format!("{{{blank}}} needs {name} to be a number, not `{text}`"))
            })?;
            filled.push_str(&written_number(sum));
        }
        rest = &rest[at + inside.len() + 2..];
    }
    filled.push_str(rest);
    Ok(filled)
}

/// A part of a sum in a blank.
#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    /// `+`, `-`, `*`, `/`, or a bracket.
    Sign(char),
}

/// The numbers, names, signs, and brackets of a sum.
fn tokens(text: &str) -> Result<Vec<Token>, String> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        let c = bytes[at] as char;
        if c.is_ascii_whitespace() {
            at += 1;
        } else if "+-*/()".contains(c) {
            tokens.push(Token::Sign(c));
            at += 1;
        } else if c.is_ascii_digit() || c == '.' {
            while at < bytes.len() && (bytes[at].is_ascii_digit() || bytes[at] == b'.') {
                at += 1;
            }
            let number = &text[start..at];
            tokens.push(Token::Number(
                number
                    .parse()
                    .map_err(|_| format!("`{number}` is not a number"))?,
            ));
        } else if c.is_ascii_alphabetic() || c == '_' {
            while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                at += 1;
            }
            tokens.push(Token::Name(text[start..at].to_string()));
        } else {
            let odd = text[start..].chars().next().unwrap_or(' ');
            return Err(format!(
                "a blank holds names, numbers, + - * /, and brackets, not `{odd}` (in `{{{text}}}`)"
            ));
        }
    }
    Ok(tokens)
}

/// What a name in a sum stands for.
type Lookup<'a> = &'a dyn Fn(&str) -> Result<f64, String>;

/// Works out a sum of numbers and names (`value` gives a name's), with `+`,
/// `-`, `*`, `/`, and brackets, as arithmetic does.
fn work_out(text: &str, value: Lookup) -> Result<f64, String> {
    let tokens = tokens(text)?;
    let mut at = 0;
    let result = sum(&tokens, &mut at, value)?;
    if at < tokens.len() {
        return Err(format!("cannot work out `{{{text}}}`"));
    }
    Ok(result)
}

fn sum(tokens: &[Token], at: &mut usize, value: Lookup) -> Result<f64, String> {
    let mut total = product(tokens, at, value)?;
    while let Some(Token::Sign(sign @ ('+' | '-'))) = tokens.get(*at) {
        *at += 1;
        let next = product(tokens, at, value)?;
        total = if *sign == '+' {
            total + next
        } else {
            total - next
        };
    }
    Ok(total)
}

fn product(tokens: &[Token], at: &mut usize, value: Lookup) -> Result<f64, String> {
    let mut total = term(tokens, at, value)?;
    while let Some(Token::Sign(sign @ ('*' | '/'))) = tokens.get(*at) {
        *at += 1;
        let next = term(tokens, at, value)?;
        total = if *sign == '*' {
            total * next
        } else {
            total / next
        };
    }
    Ok(total)
}

fn term(tokens: &[Token], at: &mut usize, value: Lookup) -> Result<f64, String> {
    let token = tokens.get(*at).ok_or("a sum in a blank ends too soon")?;
    *at += 1;
    match token {
        Token::Number(number) => Ok(*number),
        Token::Name(name) => value(name),
        Token::Sign('-') => Ok(-term(tokens, at, value)?),
        Token::Sign('(') => {
            let inside = sum(tokens, at, value)?;
            match tokens.get(*at) {
                Some(Token::Sign(')')) => {
                    *at += 1;
                    Ok(inside)
                }
                _ => Err("a bracket in a blank is left open".to_string()),
            }
        }
        Token::Sign(sign) => Err(format!("`{sign}` cannot start a number in a blank")),
    }
}

/// A worked-out number as a play is written: whole when it is whole, else
/// to nine decimals at most, so 0.1 + 0.2 reads 0.3.
fn written_number(number: f64) -> String {
    let rounded = (number * 1e9).round() / 1e9;
    if rounded.is_finite() && rounded == rounded.trunc() {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A blank is filled with its value as given, and a sum is worked out
    /// and written as a play is: the step 4 lean's timing keeps the punch
    /// on step 60 whatever the lead.
    #[test]
    fn blanks_are_filled_and_sums_worked_out() {
        let spec = "hammer:-1.3,0.95:wait={60-lead}:lean2=on:wait={lead}:punch={arm}";
        assert_eq!(
            fill(spec, &[("lead", "4"), ("arm", "right")]).unwrap(),
            "hammer:-1.3,0.95:wait=56:lean2=on:wait=4:punch=right"
        );
        let values = [("a", "1"), ("b", "2"), ("lean", "0.1")];
        assert_eq!(fill("{(a+b)/2}", &values).unwrap(), "1.5");
        assert_eq!(fill("{-a*3+b}", &values).unwrap(), "-1");
        assert_eq!(fill("{lean+0.2}", &values).unwrap(), "0.3");
        assert_eq!(fill("no blanks", &values).unwrap(), "no blanks");
        let missing = fill(spec, &[("arm", "left")]).unwrap_err();
        assert!(missing.contains("--grid lead="), "{missing}");
        assert!(fill("{60-arm}", &[("arm", "left")]).is_err());
        for broken in ["{a", "a}", "{a+}", "{(a}", "{a$b}", "{}"] {
            assert!(fill(broken, &values).is_err(), "{broken}");
        }
        let written = Written::Gesture(spec.to_string());
        let blanks: Vec<String> = written.blanks().unwrap().into_iter().collect();
        assert_eq!(blanks, ["arm", "lead"]);
        // A play with no blanks plays as written without a grid; one with
        // a blank says to give it one.
        assert!(Written::Gesture("fist:-1.3,0.95:wait=4".to_string())
            .play(&[])
            .is_ok());
        assert!(written.play(&[]).is_err());
    }

    #[test]
    fn worked_out_numbers_are_written_as_a_play_is() {
        assert_eq!(written_number(56.0), "56");
        assert_eq!(written_number(-0.0), "0");
        assert_eq!(written_number(0.1 + 0.2), "0.3");
        assert_eq!(written_number(2.5), "2.5");
    }
}
