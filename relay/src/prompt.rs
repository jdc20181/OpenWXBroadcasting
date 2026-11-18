//! Plain terminal questions for the setup tools (no extra crates; works over SSH and in any
//! Windows console).

use std::{
    io::{self, BufRead, Write},
    sync::atomic::{AtomicBool, Ordering},
};

static END_OF_INPUT: AtomicBool = AtomicBool::new(false);

/// Ask a question; Enter keeps the default (shown in brackets). With no more input (piped or
/// unattended), the default is used.
pub fn ask(question: &str, default: &str) -> String {
    if default.is_empty() {
        print!("{question}: ");
    } else {
        print!("{question} [{default}]: ");
    }
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().lock().read_line(&mut line).unwrap_or(0) == 0 {
        END_OF_INPUT.store(true, Ordering::Relaxed);
        println!();
        return default.to_owned();
    }
    let line = line.trim();
    if line.is_empty() { default.to_owned() } else { line.to_owned() }
}

/// Ask until `valid` accepts the answer; `hint` says what is expected. Without input left to
/// read, an invalid default keeps the value as it is instead of asking forever.
pub fn ask_valid(question: &str, default: &str, hint: &str, valid: impl Fn(&str) -> bool) -> String {
    loop {
        let answer = ask(question, default);
        if valid(&answer) || END_OF_INPUT.load(Ordering::Relaxed) {
            return answer;
        }
        println!("  {hint}");
    }
}

pub fn yes_no(question: &str, default: bool) -> bool {
    let answer = ask(&format!("{question} (y/n)"), if default { "y" } else { "n" });
    matches!(answer.to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Pick one of `options` by number; returns its index.
pub fn choose(question: &str, options: &[&str], default: usize) -> usize {
    println!("{question}");
    for (i, option) in options.iter().enumerate() {
        println!("  {}) {option}", i + 1);
    }
    let answer = ask_valid("Choose a number", &(default + 1).to_string(), &format!("Type a number from 1 to {}.", options.len()), |a| {
        a.parse::<usize>().is_ok_and(|n| (1..=options.len()).contains(&n))
    });
    answer.parse::<usize>().ok().filter(|n| (1..=options.len()).contains(n)).unwrap_or(default + 1) - 1
}

pub fn looks_like_email(s: &str) -> bool {
    let s = s.trim();
    let Some((user, domain)) = s.split_once('@') else { return false };
    !user.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.') && !s.contains(char::is_whitespace)
}

/// An e-mail address or an https:// website (NWS accepts either as the contact).
pub fn looks_like_contact(s: &str) -> bool {
    looks_like_email(s) || (s.starts_with("https://") && s.len() > 12 && !s.contains(char::is_whitespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contacts() {
        assert!(looks_like_contact("wx@example.org"));
        assert!(looks_like_contact("https://example.org"));
        assert!(!looks_like_contact("example"));
        assert!(!looks_like_contact("a b@c.d"));
        assert!(!looks_like_contact("x@localhost"));
    }
}

/// True once standard input has run out (piped or unattended use).
pub fn input_ended() -> bool {
    END_OF_INPUT.load(Ordering::Relaxed)
}
