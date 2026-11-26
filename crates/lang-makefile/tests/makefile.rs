use provider_api::format_testing::{assert_file_names, assert_formats, check_corpus};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_makefile::provider(), "Makefile", input)
}

/// Every Makefile of the pinned corpus (`tests/conformance/fetch-makefile-corpus.sh`) parses,
/// formats with the same tokens (the safety check) and idempotently.
///
/// `cargo test --profile corpus -p lang-makefile --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let (count, unparsed) = check_corpus(lang_makefile::provider(), "Makefile", "makefile", &[]);
    println!("{count} files, {} with syntax errors", unparsed.len());
    assert!(
        unparsed.is_empty(),
        "valid files with syntax errors: {unparsed:#?}"
    );
}

#[test]
fn grammar_fork_constructs() {
    let src = "ifeq (a,b)\n  X = $(dir)\nelse\n  override Y = $(if $(a),b,)\nendif\nS = ( \\\n\tls \\\n\t)\nall:\n\t@for m in $$($(GO) list a); do x; done\n\t$(info goal(s) $(@:%.o=%.c))\n";
    assert_eq!(fmt(src), src);
}

/// Valid makefiles the upstream grammar rejected, one smallest input each (the evidence corpus of
/// docs/ai/design/makefile.md): names with references, strays and shapes of values, statements that
/// are references, nested defines, target-specific modifiers, empty and braced substitution parts.
#[test]
fn grammar_fork_names_values_and_statements() {
    for src in [
        "OBJS-$(CONFIG_X) += a.o\n",
        "$(a) = 1\n",
        "!A := b\n",
        "A = ( a )) b\n",
        "A = case x in 0) echo;; esac\n",
        "A = $()\nB = $(subst $() $(),:,x)\n",
        "A = $([TO])\n",
        "$(error x))\n",
        "$(recurse)\n",
        "define A/$(1)\nx\nendef\n",
        "define A\n  define B\n  endef\n  y\nendef\nC = 1\n",
        "x: export A := b\n",
        "x: override A += b\n",
        "$(l:%=b/%.o): b/%.o: %.mli | b\n\tx\n",
        "A = $(c:%=a{b}) $(B:=.o) $(B:%=-Wl,%) $(B:/=)\n",
        "x:\n\t$(info )\n\t@\n",
        "x:\n\techo '$(a)#$(b)#' \\\n\t  c\n\n# after a reference\nA = 1\n",
        "ifeq ($(a), b;)\nendif\nifneq ($(A):$(B),$(C):$(D))\nendif # c\n",
    ] {
        assert_eq!(fmt(src), src, "{src:?}");
    }
    assert_eq!(fmt("ifdef A\nelse \nendif\n"), "ifdef A\nelse\nendif\n");
    // a backslash right after a `$` continues the line like any other (one blank before it)
    assert_eq!(
        fmt("A = \\\nx|$\\\ny|$\\\nz\nB = 1\n"),
        "A = \\\nx|$ \\\ny|$ \\\nz\nB = 1\n"
    );
}

#[test]
fn assignments_and_rules() {
    assert_eq!(
        fmt(
            "CC   :=  gcc   \nCFLAGS=-O2 -Wall \\\n\t-g\nexport FOO?=bar\n\n\n\n\nall : main.o   util.o|outdir  # c\n\t$(CC) -o $@ $^   \n\t@echo \"done\"; \\\n\t  ls\n\n%.o:%.c\n\t  -$(CC) -c $< -o $@\n"
        ),
        "CC := gcc   \nCFLAGS = -O2 -Wall \\\n\t-g\nexport FOO ?= bar\n\n\nall: main.o util.o | outdir # c\n\t$(CC) -o $@ $^\n\t@echo \"done\"; \\\n\t  ls\n\n%.o: %.c\n\t-$(CC) -c $< -o $@\n"
    );
}

#[test]
fn conditionals_and_define() {
    assert_eq!(
        fmt(
            "ifeq ($(OS),Windows_NT)\nEXE:=.exe\nelse ifdef X\n  EXE :=\nelse\nEXE:=.bin\nendif\ndefine RULE\n$(1): ; echo $(1)\nendef\n$(eval $(call RULE,a))\nclean:\nifdef V\n\techo verbose\n  endif\n\trm -f *.o\n"
        ),
        "ifeq ($(OS),Windows_NT)\nEXE := .exe\nelse ifdef X\n  EXE :=\nelse\nEXE := .bin\nendif\ndefine RULE\n$(1): ; echo $(1)\nendef\n$(eval $(call RULE,a))\nclean:\nifdef V\n\techo verbose\n  endif\n\trm -f *.o\n"
    );
}

#[test]
fn define_bodies_are_kept() {
    let src = "define X\n\ta  \n  b\n\nc\nendef\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn a_trailing_continuation_keeps_the_next_line_out_of_the_value() {
    let src = "A= \\\n\t-Wa \\\n\n\nB= x\n";
    assert_eq!(fmt(src), "A = \\\n\t-Wa \\\n\n\nB = x\n");
}

/// The trailing blank of a value is part of the value (`$(A)x` is `b x`): it stays.
#[test]
fn the_trailing_blank_of_a_value_is_kept() {
    let src = "A = b \nX := $(A)x\nB = c \\\n  d\t\nC = e\n";
    assert_eq!(fmt(src), src);
}

/// The blanks before the backslash that continues a line after the first are one blank in make.
#[test]
fn blanks_before_a_value_continuation_are_one() {
    assert_eq!(
        fmt("A = a   \\\n  b   \\\n  c\\\n  d \\\\\n"),
        "A = a   \\\n  b \\\n  c \\\n  d \\\\\n"
    );
}

/// A pair of backslashes at the end of a value does not continue it: nothing is inserted after it.
#[test]
fn an_escaped_backslash_does_not_continue() {
    let src = "ifdef A\nD = \\\\\n endif\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn glued_assignment_operators_are_spaced() {
    assert_eq!(
        fmt("A+=b\nB?=c\nD!=e\nx: F+=g\n"),
        "A += b\nB ?= c\nD != e\nx: F += g\n"
    );
}

/// Lines inside a conditional keep their indentation, comments and `else`, `endif` and nested
/// directives with them; the body of a rule keeps its blank lines in front.
#[test]
fn conditional_lines_keep_their_indentation() {
    let src = "ifeq ($(A),1)\n  B = 1\n   # c\n  define X\n  y\n  endef\nelse\n    C = 2\n  ifdef D\n\n  E = 3\n\n  endif\n\nendif\nx: a\n\nifdef F\n\tcmd\nendif\n";
    assert_eq!(fmt(src), src);
}

/// The blank lines that open a `define` body are part of the value.
#[test]
fn define_keeps_the_blank_lines_that_open_it() {
    let src = "define X\n\n\nfoo\nendef\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn special_rules() {
    assert_eq!(
        fmt(
            "foo: CFLAGS+=-g\n$(OBJS): %.o: %.c ; $(CC) -c $<\na b &: c\nd::e\n\techo $$HOME ; true\nvpath %.c src\n"
        ),
        "foo: CFLAGS += -g\n$(OBJS): %.o: %.c ; $(CC) -c $<\na b &: c\nd:: e\n\techo $$HOME ; true\nvpath %.c src\n"
    );
}

#[test]
fn comments_are_kept() {
    let src = "# top\nA = 1 # t\n\n# b\nall:\n\techo 1\n# between\n\techo 2\n# tail\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn matches_the_file_name() {
    assert_file_names(
        &lang_makefile::provider(),
        &["Makefile", "a/GNUmakefile", "makefile", "x.mk", "x.mak"],
        &["Makefile.in", "a.make", "mk"],
    );
}

/// The options of the mbake preset (`presets/mbake.yaml`), set one by one.
#[track_caller]
fn mbake(input: &str) -> String {
    use core_config::OptionValue::{Bool, Int, String as S};
    let options = vec![
        ("indent_style", S("tab".into())),
        ("continued_line_indent", S("first".into())),
        ("spaced_assignments", S("plain".into())),
        ("keep_comment_gap", Bool(true)),
        ("max_blank_lines", Int(1)),
    ];
    core_format::testing::assert_formats(
        &provider_api::format_testing::Formatting {
            options,
            ..provider_api::format_testing::Formatting::new(lang_makefile::provider(), "Makefile")
        },
        input,
    )
}

/// `spaced_assignments: plain`: only a plain assignment at the start of a line is spaced; an
/// indented one, one after `export`, a target-specific one, a name with `-`, a substitution
/// reference on the line and a rule-like colon keep their blanks.
#[test]
fn plain_assignments_are_spaced_the_others_stay() {
    let src = "A=1\nB   :=   2\n  C=3\nexport D=4\nx: E=5\nF-G=6\nH=$(I:%=%.o)\nJ=a:b\nK=http://a\nL = M:N\n";
    assert_eq!(
        mbake(src),
        "A = 1\nB := 2\n  C=3\nexport D=4\nx: E=5\nF-G=6\nH=$(I:%=%.o)\nJ=a:b\nK = http://a\nL = M:N\n"
    );
    // the default spaces all of them
    assert_eq!(fmt("  C=3\nF-G=6\nJ=a:b\n"), "C = 3\nF-G = 6\nJ = a:b\n");
}

/// A line that stays as written keeps the blanks around its colon and in its value.
#[test]
fn a_target_specific_assignment_keeps_its_blanks() {
    let src = "t:   CMD  =   x   y \\\n  z\n";
    assert_eq!(mbake(src), src);
}

/// `continued_line_indent: first`: every line after the first takes the indentation of the second;
/// a rule's or recipe's second line that starts with a space gets a tab; one blank before each
/// backslash after the first line.
#[test]
fn continued_lines_take_the_indentation_of_the_second() {
    assert_eq!(
        mbake("A = a \\\n\t\tb   \\\n  c\\\n\td\n"),
        "A = a \\\n\t\tb \\\n\t\tc \\\n\t\td\n"
    );
    assert_eq!(mbake("x: a \\\n  b \\\n\tc\n"), "x: a \\\n\tb \\\n\tc\n");
    assert_eq!(
        mbake("x:\n\tcmd a \\\n\t\tb \\\n    c\n"),
        "x:\n\tcmd a \\\n\t\tb \\\n\t\tc\n"
    );
    // a backslash alone on a line gets a blank in front of it
    assert_eq!(mbake("A = a \\\n\t\\\n\tb\n"), "A = a \\\n\t \\\n\tb\n");
    // the last line of a variable may be empty
    assert_eq!(
        mbake("A = a \\\n\tb \\\n\nB = c\n"),
        "A = a \\\n\tb \\\n\nB = c\n"
    );
}

/// A recipe command with a shell block keyword and a line that starts inside a quote keep their
/// indentation.
#[test]
fn shell_blocks_and_quotes_keep_their_continued_lines() {
    let src = "x:\n\tif a; then \\\n\t  b; \\\n\tfi\n";
    assert_eq!(mbake(src), src);
    let src = "x:\n\techo 'a \\\n    b' \\\n\t\tc\n";
    assert_eq!(mbake(src), src);
}

#[test]
fn the_default_leaves_continued_lines_as_written() {
    let src = "A = a \\\n\t\tb \\\n  c\nx: a \\\n  b \\\n\tc\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn keep_comment_gap_keeps_the_blanks_before_a_comment() {
    assert_eq!(
        mbake("x: a   # c\nifdef A\nelse   # d\nendif\n"),
        "x: a   # c\nifdef A\nelse   # d\nendif\n"
    );
    assert_eq!(fmt("x: a   # c\n"), "x: a # c\n");
}

/// A command line in the body of a conditional that no rule contains is not a recipe line: it keeps
/// its blanks.
#[test]
fn a_tab_line_in_a_conditional_keeps_its_blanks() {
    let src = "ifeq (a,b)\n\t\tX=1\n\t  Y=2\nendif\n";
    assert_eq!(fmt(src), src);
}

/// The safety check compares what `value` returns: the blank at the end of a value is part of it,
/// the blanks before it and the indentation of continued lines are not.
#[test]
fn the_meaning_check_sees_the_trailing_blank_of_a_value() {
    use core_syntax::SyntaxElement;
    use provider_api::LanguageProvider;
    let provider = lang_makefile::provider();
    let value_of = |text: &str| {
        let file = provider_api::SourceFile::new("Makefile", 1, text);
        let tree = provider.parse(&file).unwrap().tree;
        let token = tree
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| {
                provider
                    .grammar()
                    .def(t.kind())
                    .is_some_and(|d| d.name == "Value")
            })
            .unwrap();
        provider
            .formatter()
            .value(&SyntaxElement::Token(token))
            .to_string()
    };
    assert_eq!(value_of("A =  b \n"), "b ");
    assert_ne!(value_of("A = b \n"), value_of("A = b\n"));
    assert_eq!(
        value_of("A = b \\\n   c \\\n d\n"),
        value_of("A = b \\\nc \\\n\td\n")
    );
}

/// A continued command line inside a conditional keeps the spaces of its second line: a tab there
/// would end the command for the parser.
#[test]
fn a_continued_command_in_a_conditional_keeps_its_spaces() {
    let src = "x: y\nifeq (a,b)\nifneq (c,d)\n\tcmd \\\n   arg\nendif\nendif\n";
    assert_eq!(mbake(src), src);
}

/// The blank lines before an assignment do not hide its substitution reference.
#[test]
fn a_substitution_reference_counts_after_a_blank_line() {
    let src = "A = b\n\nM   = $(L:%=d)\n";
    assert_eq!(mbake(src), src);
}

/// The line after `ifeq "a" "b"` is a statement of its own, also when it looks like a word with `=`.
#[test]
fn the_line_after_a_quoted_ifeq_stays_a_line() {
    let src = "ifeq \"a\" \"b\"\nX=1\nendif\n";
    assert_eq!(fmt(src), "ifeq \"a\" \"b\"\nX = 1\nendif\n");
}
