//! The engine with rules written for the test: scopes, dispatch, layers, options, and the project
//! phase. The language is JSON, because its parser is the smallest.

use core_config::{Level, OptionValue, ResolvedRule, Source};
use core_rules::*;
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};
use provider_api::{LanguageProvider, SourceFile};
use std::collections::BTreeMap;
use std::path::Path;

fn kind(name: &str) -> SyntaxKind {
    lang_json::provider()
        .grammar()
        .kind_by_name(name)
        .expect("a JSON kind")
}

/// Reports every node of one kind, every file, and every body.
struct Probe {
    meta: &'static RuleMeta,
    kinds: Vec<SyntaxKind>,
}

static PROBE: RuleMeta = RuleMeta {
    id: "probe",
    category: Category::Style,
    why: "",
    source: "",
    description: "",
    example: "",
    default_level: Level::Warn,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[OptionDecl {
        name: "word",
        default: Opt::Str("hit"),
        check: Check::Any,
        description: "",
    }],
};

impl Rule for Probe {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &self.kinds
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let word = ctx.options.string("word").to_string();
        ctx.report(node.text_range(), format!("node {word}"));
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        ctx.report(
            TextRange::up_to(0.into()),
            format!("file {}", file.text.len()),
        );
    }

    fn check_body(&self, body: &BodyContext<'_>, ctx: &mut RuleContext<'_>) {
        ctx.report(body.node.text_range(), "body");
    }

    fn collect(&self, file: &FileContext<'_>, facts: &mut ProjectFacts) {
        facts.add(
            TextRange::up_to(0.into()),
            "path",
            file.path.to_string_lossy(),
        );
    }

    fn finish(&self, facts: &ProjectFacts, ctx: &mut ProjectContext<'_>) {
        let all: Vec<_> = facts.of_rule("probe").collect();
        if all.len() > 1 {
            ctx.report(&all[0].file, all[0].range, format!("{} files", all.len()));
        }
    }
}

fn run(text: &str, rule: &Probe, collect: bool, bodies: &[SyntaxKind]) -> FileReport {
    let provider = lang_json::provider();
    let parsed = provider.parse(&SourceFile::new("a.json", 1, text)).unwrap();
    let lines = LineIndex::new(text);
    let file = FileContext {
        path: Path::new("a.json"),
        text,
        root: &parsed.tree,
        grammar: provider.grammar(),
        lines: &lines,
        body_kinds: bodies,
        hooks: provider.rule_hooks(),
        embedded: &[],
        cells: &[],
        semantics: None,
    };
    let active = [ActiveRule {
        rule,
        severity: Severity::Warning,
        options: RuleOptions::resolve(rule.meta.options, &BTreeMap::new()).unwrap(),
    }];
    run_file(&file, &active, collect)
}

fn messages(report: &FileReport) -> Vec<String> {
    report
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn nodes_go_only_to_rules_subscribed_to_their_kind() {
    let probe = Probe {
        meta: &PROBE,
        kinds: vec![kind("Member")],
    };
    let report = run("{\"a\": 1, \"b\": [2]}", &probe, false, &[]);
    assert_eq!(messages(&report), ["file 18", "node hit", "node hit"]);
    assert!(report.facts.is_empty());
}

#[test]
fn body_scope_is_called_once_per_body_node() {
    let probe = Probe {
        meta: &PROBE,
        kinds: vec![],
    };
    let report = run("[[1], [2], {}]", &probe, false, &[kind("Array")]);
    let bodies = messages(&report).iter().filter(|m| *m == "body").count();
    assert_eq!(bodies, 3, "the outer array and the two inner ones");
}

#[test]
fn diagnostics_are_sorted_by_position() {
    let probe = Probe {
        meta: &PROBE,
        kinds: vec![kind("Literal"), kind("Array")],
    };
    let report = run("[1, [2]]", &probe, false, &[]);
    let starts: Vec<u32> = report
        .diagnostics
        .iter()
        .map(|d| d.range.start().into())
        .collect();
    assert!(starts.windows(2).all(|w| w[0] <= w[1]), "{starts:?}");
}

#[test]
fn project_phase_sees_the_facts_of_all_files() {
    let probe = Probe {
        meta: &PROBE,
        kinds: vec![],
    };
    let first = run("{}", &probe, true, &[]);
    let mut facts = first.facts.clone();
    facts.extend(&run("[]", &probe, true, &[]).facts);
    let active = [ActiveRule {
        rule: &probe,
        severity: Severity::Error,
        options: RuleOptions::default(),
    }];
    let out = run_project(&active, &facts);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].1.message, "2 files");
    assert_eq!(out[0].1.severity, Severity::Error);
    // One file alone has nothing to report across.
    assert!(run_project(&active, &first.facts).is_empty());
}

fn configured(level: Level, options: &[(&str, OptionValue)]) -> BTreeMap<String, ResolvedRule> {
    BTreeMap::from([(
        "probe".to_string(),
        ResolvedRule {
            level,
            options: options
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            source: Source::Default,
        },
    )])
}

#[test]
fn activation_follows_level_options_and_layers() {
    let probe = Probe {
        meta: &PROBE,
        kinds: vec![],
    };
    let rules: [&dyn Rule; 1] = [&probe];
    let on = |cfg| activate(rules, &cfg, |_| true).unwrap();
    // Default level when the config is silent.
    assert_eq!(on(BTreeMap::new())[0].severity, Severity::Warning);
    assert_eq!(
        on(configured(Level::Error, &[]))[0].severity,
        Severity::Error
    );
    assert!(on(configured(Level::Off, &[])).is_empty());
    let word = OptionValue::String("custom".into());
    assert_eq!(
        on(configured(Level::Warn, &[("word", word)]))[0]
            .options
            .string("word"),
        "custom"
    );

    let bad = activate(
        rules,
        &configured(Level::Warn, &[("wordd", OptionValue::Bool(true))]),
        |_| true,
    );
    let message = bad.err().unwrap().message;
    assert!(
        message.contains("rule `probe`") && message.contains("did you mean `word`"),
        "{message}"
    );
    let wrong = activate(
        rules,
        &configured(Level::Warn, &[("word", OptionValue::Bool(true))]),
        |_| true,
    );
    assert!(wrong.err().unwrap().message.contains("must be a string"));
}

#[test]
fn a_rule_whose_layer_is_missing_is_skipped_not_failed() {
    static NEEDS_BINDING: RuleMeta = RuleMeta {
        requires: &[Layer::Binding],
        uses: &[],
        ..RuleMeta {
            id: "needs-binding",
            category: Category::Correctness,
            why: "",
            source: "",
            description: "",
            example: "",
            default_level: Level::Error,
            fix: None,
            requires: &[],
            uses: &[],
            options: &[],
        }
    };
    let probe = Probe {
        meta: &NEEDS_BINDING,
        kinds: vec![],
    };
    let rules: [&dyn Rule; 1] = [&probe];
    assert!(
        activate(rules, &BTreeMap::new(), |_| false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        activate(rules, &BTreeMap::new(), |l| l == Layer::Binding)
            .unwrap()
            .len(),
        1
    );
}

/// A member of a cluster: only the first active one runs, and reports for all in their names.
struct Member {
    meta: &'static RuleMeta,
    kinds: Vec<SyntaxKind>,
}

static FIRST: RuleMeta = RuleMeta {
    id: "first",
    ..PROBE
};
static SECOND: RuleMeta = RuleMeta {
    id: "second",
    ..PROBE
};

impl Rule for Member {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn cluster(&self) -> Option<&'static str> {
        Some("pair")
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &self.kinds
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        for member in ctx.members().to_vec() {
            ctx.report_as(member, node.text_range(), format!("by {}", self.meta.id));
        }
    }
}

#[test]
fn a_cluster_runs_its_first_active_rule_and_reports_for_each_member() {
    let provider = lang_json::provider();
    let text = "{\"a\": 1}";
    let parsed = provider.parse(&SourceFile::new("a.json", 1, text)).unwrap();
    let lines = LineIndex::new(text);
    let file = FileContext {
        path: Path::new("a.json"),
        text,
        root: &parsed.tree,
        grammar: provider.grammar(),
        lines: &lines,
        body_kinds: &[],
        hooks: provider.rule_hooks(),
        embedded: &[],
        cells: &[],
        semantics: None,
    };
    let member = |meta| Member {
        meta,
        kinds: vec![kind("Member")],
    };
    let (a, b) = (member(&FIRST), member(&SECOND));
    let both = [
        ActiveRule {
            rule: &a,
            severity: Severity::Warning,
            options: RuleOptions::default(),
        },
        ActiveRule {
            rule: &b,
            severity: Severity::Error,
            options: RuleOptions::default(),
        },
    ];
    let report = run_file(&file, &both, false);
    let got: Vec<_> = report
        .diagnostics
        .iter()
        .map(|d| (d.rule.as_str(), d.severity, d.message.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("first", Severity::Warning, "by first"),
            ("second", Severity::Error, "by first")
        ]
    );
    // The second alone leads for itself only.
    let report = run_file(&file, &both[1..], false);
    assert_eq!(messages(&report), ["by second"]);
}
