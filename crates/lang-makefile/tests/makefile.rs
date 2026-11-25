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
        "A = \\\nx|$\\\ny|$\\\nz\nB = 1\n",
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
        "ifeq ($(OS),Windows_NT)\nEXE := .exe\nelse ifdef X\n  EXE :=\nelse\nEXE := .bin\nendif\ndefine RULE\n$(1): ; echo $(1)\nendef\n$(eval $(call RULE,a))\nclean:\nifdef V\n\techo verbose\nendif\n\trm -f *.o\n"
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

/// The blanks before the backslash that continues a value are one blank in make.
#[test]
fn blanks_before_a_value_continuation_are_one() {
    assert_eq!(fmt("A = a   \\\n  b \\\\\n"), "A = a \\\n  b \\\\\n");
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
