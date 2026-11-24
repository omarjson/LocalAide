//! Read-only shell commands that may run without an approval prompt.
//!
//! This classifier is deliberately conservative. A command skips approval
//! only when *every* segment joined by `&&`, `||`, `;`, `|` or a newline is
//! inert: a known read-only program, with arguments that stay inside the
//! workspace, no output redirect to a real file, and no shell expansion we
//! cannot read (`$(...)`, backticks, `<(...)`, and so on).
//!
//! It never grants permission by itself: callers combine it with the policy
//! rules, and an explicit deny rule always wins over this fast path.

/// The classifier's answer. Named so call sites can log or branch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnly {
    /// Every segment is inert; no approval prompt is needed.
    Yes,
    /// At least one segment changes something or cannot be read safely;
    /// keep the approval prompt.
    No,
}

/// True when `command` can skip the approval prompt.
pub fn can_skip_approval(command: &str) -> bool {
    classify(command) == ReadOnly::Yes
}

/// Classify a command line. Empty input and anything with an opaque shell
/// construct are never considered inert.
pub fn classify(command: &str) -> ReadOnly {
    let trimmed = command.trim();
    if trimmed.is_empty() || has_opaque(trimmed) {
        return ReadOnly::No;
    }
    let inert = segments(trimmed)
        .iter()
        .all(|segment| segment_is_inert(segment.as_str()));
    if inert { ReadOnly::Yes } else { ReadOnly::No }
}

/// Shell constructs whose effect cannot be read from the literal command.
fn has_opaque(command: &str) -> bool {
    command.contains("$(")
        || command.contains("${")
        || command.contains('`')
        || command.contains("<(")
        || command.contains(">(")
        || command.contains('\n')
        || command.contains('\r')
}

/// Split into segments at `&&`, `||`, `;`, `|` and a single `&`. Quotes and
/// backslashes hide separators. A lone `&` (background job) becomes its own
/// inert-free marker so the whole command keeps its prompt.
fn segments(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(c) = chars.next() {
        if c == '\\' && quote != Some('\'') {
            current.push(c);
            if let Some(next) = chars.next() {
                current.push(next);
            }
            continue;
        }
        if let Some(q) = quote {
            current.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                current.push(c);
            }
            ';' | '|' | '&' => {
                if chars.peek() == Some(&c) {
                    chars.next();
                } else if c == '&' {
                    // A background job is not something we reason about.
                    out.push(std::mem::take(&mut current));
                    out.push("&".to_string());
                    continue;
                }
                out.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    out.push(current);
    out.into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Words of one segment, plus whether it redirects output to a real file.
/// Quotes are removed; `2>&1`, `>&2` and `>/dev/null` are not writes.
fn words_and_redirect(segment: &str) -> (Vec<String>, bool) {
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut redirect_target = false;
    let mut writes_file = false;
    let mut chars = segment.chars().peekable();

    let finish_word = |word: &mut String,
                       in_word: &mut bool,
                       redirect_target: &mut bool,
                       writes_file: &mut bool,
                       words: &mut Vec<String>| {
        if !*in_word {
            return;
        }
        if *redirect_target {
            let target = word.trim().to_ascii_lowercase();
            if target != "/dev/null" && target != "nul" && !target.is_empty() {
                *writes_file = true;
            }
            *redirect_target = false;
            word.clear();
        } else {
            words.push(std::mem::take(word));
        }
        *in_word = false;
    };

    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                for q in chars.by_ref() {
                    if q == '\'' {
                        break;
                    }
                    word.push(q);
                }
            }
            '"' => {
                in_word = true;
                while let Some(q) = chars.next() {
                    if q == '"' {
                        break;
                    }
                    if q == '\\' {
                        if let Some(next) = chars.next() {
                            word.push(next);
                        }
                    } else {
                        word.push(q);
                    }
                }
            }
            '\\' => {
                in_word = true;
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            '>' => {
                if in_word && (word == "2" || word == "1") {
                    word.clear();
                    in_word = false;
                } else {
                    finish_word(
                        &mut word,
                        &mut in_word,
                        &mut redirect_target,
                        &mut writes_file,
                        &mut words,
                    );
                }
                if chars.peek() == Some(&'>') {
                    chars.next();
                }
                while chars.peek() == Some(&' ') {
                    chars.next();
                }
                if chars.peek() == Some(&'&') {
                    chars.next();
                    while chars
                        .peek()
                        .is_some_and(|c| c.is_ascii_digit() || *c == '-')
                    {
                        chars.next();
                    }
                } else {
                    redirect_target = true;
                }
            }
            '<' => {
                // Input redirection may read from anywhere on the machine;
                // treat it as unsafe rather than trying to resolve the path.
                writes_file = true;
                finish_word(
                    &mut word,
                    &mut in_word,
                    &mut redirect_target,
                    &mut writes_file,
                    &mut words,
                );
            }
            c if c.is_whitespace() => {
                finish_word(
                    &mut word,
                    &mut in_word,
                    &mut redirect_target,
                    &mut writes_file,
                    &mut words,
                );
            }
            _ => {
                in_word = true;
                word.push(c);
            }
        }
    }
    finish_word(
        &mut word,
        &mut in_word,
        &mut redirect_target,
        &mut writes_file,
        &mut words,
    );
    if redirect_target {
        writes_file = true;
    }
    (words, writes_file)
}

/// Argument that points outside the working folder. Flags are safe unless a
/// `--key=value` value is itself an outside path.
fn reaches_outside(word: &str) -> bool {
    let w = word.trim_matches(|c| c == '"' || c == '\'');
    if w.is_empty() {
        return false;
    }
    if w.starts_with('-') {
        return w
            .split_once('=')
            .is_some_and(|(_, value)| reaches_outside(value));
    }
    w.starts_with('/')
        || w.starts_with('~')
        || w.starts_with('$')
        || w.starts_with('%')
        || w.starts_with("\\\\")
        || w.split(['/', '\\']).any(|part| part == "..")
        || (w.len() >= 2 && w.as_bytes()[1] == b':' && w.as_bytes()[0].is_ascii_alphabetic())
}

/// Programs that only read, whatever their arguments. Output redirection and
/// paths outside the folder are handled separately.
const READ_ONLY: &[&str] = &[
    "ls",
    "dir",
    "pwd",
    "cat",
    "type",
    "head",
    "tail",
    "wc",
    "echo",
    "grep",
    "egrep",
    "fgrep",
    "rg",
    "tree",
    "du",
    "df",
    "stat",
    "file",
    "which",
    "where",
    "whoami",
    "hostname",
    "date",
    "uname",
    "sort",
    "uniq",
    "cut",
    "diff",
    "cmp",
    "basename",
    "dirname",
    "realpath",
    "readlink",
    "md5sum",
    "sha1sum",
    "sha256sum",
    "shasum",
    "cksum",
    "jq",
    "true",
    "false",
    "test",
    "nl",
    "column",
    "printenv",
    "get-childitem",
    "gci",
    "get-content",
    "gc",
    "get-location",
    "select-string",
    "findstr",
    "more",
    "id",
    "uptime",
    "free",
    "cal",
    "seq",
];

/// Git subcommands that only read. Anything that writes history or the
/// working tree (commit, push, reset, clean, checkout with `--`, ...) is
/// absent, so it keeps the approval prompt.
const GIT_READ: &[&str] = &[
    "status",
    "log",
    "diff",
    "show",
    "rev-parse",
    "ls-files",
    "blame",
    "describe",
    "shortlog",
    "grep",
    "ls-tree",
    "cat-file",
    "reflog",
    "whatchanged",
    "count-objects",
];

/// Decide one segment. Unknown programs are never inert.
fn segment_is_inert(segment: &str) -> bool {
    let (words, writes_file) = words_and_redirect(segment);
    if writes_file {
        return false;
    }
    // Ignore leading `VAR=value` assignments; they only set a variable.
    let program_at = words
        .iter()
        .position(|w| !(w.contains('=') && !w.starts_with('=') && !w.starts_with('-')))
        .unwrap_or(0);
    let program = words
        .get(program_at)
        .map(|w| {
            w.rsplit(['/', '\\'])
                .next()
                .unwrap_or(w)
                .trim_end_matches(".exe")
                .to_ascii_lowercase()
        })
        .unwrap_or_default();
    if program.is_empty() {
        return false;
    }
    let args: Vec<String> = words
        .iter()
        .skip(program_at + 1)
        .map(|w| w.to_ascii_lowercase())
        .collect();
    if words
        .iter()
        .skip(program_at + 1)
        .any(|w| reaches_outside(w.as_str()))
    {
        return false;
    }
    let has = |flag: &str| args.iter().any(|a| a == flag);
    match program.as_str() {
        "git" => {
            let sub = args
                .iter()
                .find(|a| !a.starts_with('-'))
                .map(String::as_str)
                .unwrap_or("");
            match sub {
                s if GIT_READ.contains(&s) => true,
                "branch" => args.iter().skip(1).all(|a| {
                    matches!(
                        a.as_str(),
                        "-a" | "-v" | "-vv" | "--list" | "-r" | "--all" | "--show-current"
                    )
                }),
                "remote" => args
                    .iter()
                    .skip(1)
                    .all(|a| matches!(a.as_str(), "-v" | "show" | "get-url")),
                "tag" => args.len() == 1 || args.iter().skip(1).all(|a| a == "-l" || a == "--list"),
                "config" => has("--get") || has("--list") || has("-l"),
                "stash" => args.get(1).is_some_and(|a| a == "list" || a == "show"),
                _ => false,
            }
        }
        "find" => !args.iter().any(|a| {
            matches!(
                a.as_str(),
                "-delete"
                    | "-exec"
                    | "-execdir"
                    | "-ok"
                    | "-okdir"
                    | "-fprint"
                    | "-fprintf"
                    | "-fls"
            )
        }),
        "sed" => !args
            .iter()
            .any(|a| a.starts_with("-i") || a == "--in-place"),
        "awk" | "gawk" => !args
            .iter()
            .any(|a| a.contains("system(") || a.contains('>')),
        p if READ_ONLY.contains(&p) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{ReadOnly, can_skip_approval, classify};

    fn skips(c: &str) {
        assert_eq!(classify(c), ReadOnly::Yes, "{c} should skip approval");
        assert!(can_skip_approval(c), "{c} should skip approval");
    }

    fn asks(c: &str) {
        assert_eq!(classify(c), ReadOnly::No, "{c} should keep the prompt");
        assert!(!can_skip_approval(c), "{c} should keep the prompt");
    }

    #[test]
    fn simple_read_commands_skip_approval() {
        for command in [
            "ls",
            "ls -la",
            "pwd",
            "cat notes.txt",
            "git status",
            "git log --oneline -5",
            "git diff HEAD~1",
            "find . -name '*.rs'",
            "head -20 README.md",
            "grep -rn TODO src",
        ] {
            skips(command);
        }
    }

    #[test]
    fn inert_compound_commands_skip_approval() {
        for command in [
            "ls -la && git status",
            "pwd ; git log --oneline -3",
            "cat notes.txt && find . -name '*.md'",
            "ls && pwd && git status",
            "git status || git log -2",
            "ls -la | grep rs",
        ] {
            skips(command);
        }
    }

    #[test]
    fn a_compound_command_with_rm_must_not_skip() {
        for command in [
            "ls && rm -f notes.txt",
            "pwd; rm build.log",
            "git status && rm -rf build",
            "ls && git status && rm x",
            "rm -f x && ls",
        ] {
            asks(command);
        }
    }

    #[test]
    fn unsafe_reads_keep_the_prompt() {
        for command in [
            "cat ../secret.txt",
            "cat /etc/passwd",
            "ls ~",
            "git push",
            "git commit -m x",
            "find . -delete",
            "find . -exec rm {} \\;",
            "echo hi > out.txt",
            "echo \"$(whoami)\"",
            "ls `pwd`",
            "touch a.txt",
            "curl https://example.com",
            "unknown-program",
            "",
        ] {
            asks(command);
        }
    }

    #[test]
    fn quotes_hide_operators_but_not_expansions() {
        skips("echo 'a > b'");
        skips("grep \"x | sh\" file.txt");
        asks("echo hi>a.txt");
        asks("ls $HOME");
        asks("ls &");
    }
}
