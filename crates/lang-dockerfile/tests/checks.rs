//! The checks that keep a format from changing what Docker or the shell reads: the shell text
//! compared by the Shell provider, the rewritten pairs and arrays compared by value.

use core_config::OptionValue;
use core_format::{EmbedMode, Embedder, Forward, safety};
use provider_api::format_testing::Formatting;
use provider_api::{LanguageProvider, SourceFile};

/// The Shell provider as the embedder of the shell text: its tree, its values, its comments.
struct Shell;

impl Embedder for Shell {
    fn format(
        &self,
        _: &str,
        _: EmbedMode,
        _: &str,
        _: (usize, bool),
        _: Forward<'_>,
    ) -> Option<String> {
        None
    }

    fn same_code(
        &self,
        _: &str,
        _: EmbedMode,
        (before, after): (&str, &str),
        _: (usize, bool),
        _: Forward<'_>,
    ) -> Result<(), String> {
        let provider = lang_shell::provider();
        let options = provider.option_schema().defaults();
        let parse = |text: &str| {
            let mut file = SourceFile::new("a.sh", 1, text);
            file.language_version = Some(EmbedMode::Continued.name().to_string());
            let parsed = provider.parse(&file).map_err(|e| e.to_string())?;
            match parsed.errors.first() {
                Some(e) => Err(e.message.clone()),
                None => Ok(parsed.tree),
            }
        };
        let tree = parse(before)?;
        safety::verify(
            provider.grammar(),
            provider.formatter(),
            &options,
            &tree,
            after,
            parse,
        )
        .map_err(|e| e.to_string())
    }
}

/// The safety check of the Dockerfile format on `after`, written by hand as the output for `before`.
fn check(before: &str, after: &str) -> Result<(), String> {
    let provider = lang_dockerfile::provider();
    let options = provider.option_schema().defaults();
    let parse = |text: &str| {
        let parsed = provider
            .parse(&SourceFile::new("Dockerfile", 1, text))
            .map_err(|e| e.to_string())?;
        match parsed.errors.first() {
            Some(e) => Err(e.message.clone()),
            None => Ok(parsed.tree),
        }
    };
    let tree = parse(before)?;
    safety::verify_with(
        provider.grammar(),
        provider.formatter(),
        &options,
        &tree,
        after,
        parse,
        Some(&Shell),
    )
    .map_err(|e| e.to_string())
}

#[test]
fn shell_text_that_reads_the_same_passes() {
    check("RUN a  \\\n  && b\n", "RUN a \\\n    && b\n").unwrap();
    // (a continuation line that only holds blanks is dropped by Docker)
    check("RUN a \\\n\n  b\n", "RUN a \\\n  b\n").unwrap();
}

#[test]
fn a_lost_blank_between_words_is_refused() {
    assert!(check("RUN rm a b\n", "RUN rm ab\n").is_err());
}

#[test]
fn a_changed_blank_inside_a_quote_is_refused() {
    assert!(check("RUN echo \"a  b\"\n", "RUN echo \"a b\"\n").is_err());
}

#[test]
fn a_lost_or_changed_comment_line_is_refused() {
    let before = "RUN a \\\n  # note\n  && b\n";
    assert!(check(before, "RUN a \\\n  && b\n").is_err());
    assert!(check(before, "RUN a \\\n  # other\n  && b\n").is_err());
    // (a line that starts with `#` inside a quote is a comment line for Docker too)
    let before = "RUN echo \"x \\\n# y\n z\"\n";
    assert!(check(before, "RUN echo \"x \\\n z\"\n").is_err());
    check(before, before).unwrap();
}

fn pairs() -> Formatting {
    let mut f = Formatting::new(lang_dockerfile::provider(), "Dockerfile");
    f.options = vec![
        ("key_value_pairs", OptionValue::Bool(true)),
        ("json_copy_as_words", OptionValue::Bool(true)),
    ];
    f
}

#[track_caller]
fn rewritten(input: &str) -> String {
    core_format::testing::assert_formats(&pairs(), input)
}

#[test]
fn a_legacy_pair_becomes_one_word_of_the_same_value() {
    assert_eq!(rewritten("ENV A b c\n"), "ENV A=\"b c\"\n");
    // (the value has a `$`: the quotes keep its meaning)
    assert_eq!(rewritten("ENV A $B c\n"), "ENV A=\"$B c\"\n");
    assert_eq!(rewritten("LABEL a b\n"), "LABEL a=b\n");
    // (an escape in the value: the value Docker reads would change or is unclear)
    assert_eq!(rewritten("ENV A a\\ b\n"), "ENV A a\\ b\n");
}

#[test]
fn the_escape_directive_of_the_file_is_the_escape_of_the_values() {
    let input = "# escape=`\nFROM x\nENV A b`c d\n";
    assert_eq!(rewritten(input), input);
}

#[test]
fn a_hash_line_inside_a_quote_of_continued_text_is_a_comment_for_docker_and_for_the_check() {
    // Docker drops a line that starts with `#` from a continued instruction, also inside a quote:
    // `echo "x \` + `# y` + ` z"` reads as `echo "x  z"`. The formatter keeps the line where it
    // is, and the safety check reads the text the same way: dropping the line is a lost comment
    // line (refused), keeping it passes.
    let input = "RUN echo \"x \\\n# y\n z\"\n";
    let f = Formatting::new(lang_dockerfile::provider(), "Dockerfile");
    let out = core_format::testing::assert_formats(&f, input);
    assert_eq!(out, input);
    assert!(check(input, "RUN echo \"x \\\n z\"\n").is_err());
    check(input, &out).unwrap();
}

#[test]
fn a_json_array_becomes_words_when_each_is_one() {
    assert_eq!(rewritten("COPY [\"a\", \"b\"]\n"), "COPY a b\n");
    assert_eq!(
        rewritten("COPY [\"a b\", \"c\"]\n"),
        "COPY [\"a b\", \"c\"]\n"
    );
    assert_eq!(
        rewritten("COPY [\"$A\", \"c\"]\n"),
        "COPY [\"$A\", \"c\"]\n"
    );
}
