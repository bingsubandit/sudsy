//! The words cmd reads at the start of a command: its internal commands, the keywords of
//! `IF`/`FOR`, and how each takes what follows.
//!
//! concept: none (data: the command words of cmd)

/// What a command word is to cmd.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Word {
    /// An internal command taking blank-separated arguments.
    Internal,
    /// An internal command whose rest of the line is text (`echo`, `title`, `prompt`).
    Text,
    Set,
    If,
    For,
    Rem,
}

/// `word` is an internal command or keyword, read without case; `None`: a program or a file.
pub fn word(word: &[u8]) -> Option<Word> {
    let mut buf = [0u8; 10];
    let low = buf.get_mut(..word.len())?;
    low.copy_from_slice(word);
    low.make_ascii_lowercase();
    Some(match &*low {
        b"echo" | b"title" | b"prompt" => Word::Text,
        b"set" => Word::Set,
        b"if" => Word::If,
        b"for" => Word::For,
        b"rem" => Word::Rem,
        b"assoc" | b"break" | b"call" | b"cd" | b"chcp" | b"chdir" | b"cls" | b"color"
        | b"copy" | b"date" | b"del" | b"dir" | b"endlocal" | b"erase" | b"exit" | b"ftype"
        | b"goto" | b"md" | b"mkdir" | b"mklink" | b"move" | b"path" | b"pause" | b"popd"
        | b"pushd" | b"rd" | b"ren" | b"rename" | b"rmdir" | b"setlocal" | b"shift" | b"start"
        | b"time" | b"type" | b"ver" | b"verify" | b"vol" => Word::Internal,
        _ => return None,
    })
}

/// Does cmd end the command word at `.` (`echo.`, `cd..`)? Other commands end it at the characters of [`ends_word`].
pub fn dot_ends(word: &[u8]) -> bool {
    ["echo", "cd", "chdir", "rem"]
        .iter()
        .any(|w| w.as_bytes().eq_ignore_ascii_case(word))
}

/// The characters that end the name of an internal command when they follow it directly (`dir/b`, `goto:eof`, `echo(`).
pub fn ends_word(c: u8) -> bool {
    matches!(
        c,
        b'/' | b'\\' | b':' | b'+' | b',' | b';' | b'=' | b'[' | b']' | b'('
    )
}

/// The comparison words of `IF` (`equ`, `neq`, ...).
pub fn comparison(word: &[u8]) -> bool {
    ["equ", "neq", "lss", "leq", "gtr", "geq"]
        .iter()
        .any(|w| w.as_bytes().eq_ignore_ascii_case(word))
}

/// The words that start the test of an `IF` and take one operand.
pub fn test_word(word: &[u8]) -> bool {
    ["exist", "defined", "errorlevel", "cmdextversion"]
        .iter()
        .any(|w| w.as_bytes().eq_ignore_ascii_case(word))
}
