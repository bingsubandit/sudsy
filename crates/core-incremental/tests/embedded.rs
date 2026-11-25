//! Code of one language inside another: linted with its own rules (positions, suppressions and
//! config in the host file), formatted by its own formatter when the host asks.

use core_config::{ConfigChain, EditorConfig};
use core_incremental::{Database, File, format, lint};
use provider_api::LanguageProvider;
use std::path::PathBuf;

fn db(config: &str) -> Database {
    let providers: Vec<Box<dyn LanguageProvider>> = vec![
        Box::new(lang_html::provider()),
        Box::new(lang_json::provider()),
        Box::new(lang_js::provider()),
        Box::new(lang_css::provider(lang_css::Dialect::Css)),
        Box::new(lang_css::provider(lang_css::Dialect::Scss)),
        Box::new(lang_yaml::provider()),
        Box::new(lang_markdown::provider()),
        Box::new(lang_dockerfile::provider()),
        Box::new(lang_shell::provider()),
        Box::new(lang_sql::provider()),
        Box::new(lang_python::provider()),
    ];
    let mut db = Database::new(providers);
    db.set_config(ConfigChain::parse(config));
    db
}

fn file(db: &Database, path: &str, text: &str) -> File {
    File::new(
        db,
        PathBuf::from(path),
        1,
        text.into(),
        false,
        EditorConfig::default(),
    )
}

/// `(rule, text at the start of its range)` of the file's diagnostics.
fn found(db: &Database, path: &str, text: &str) -> Vec<(String, String)> {
    let linted = lint(db, file(db, path, text)).as_ref().unwrap();
    let at = |d: &core_rules::Diagnostic| {
        text[usize::from(d.range.start())..]
            .chars()
            .take(4)
            .collect()
    };
    linted
        .diagnostics
        .iter()
        .map(|d| (d.rule.clone(), at(d)))
        .collect()
}

fn formatted(db: &Database, path: &str, text: &str) -> String {
    format(db, file(db, path, text)).as_ref().unwrap().clone()
}

#[test]
fn run_is_linted_as_shell_at_host_positions() {
    let dockerfile =
        "dockerfile:\n  lint:\n    rules:\n      use-workdir-not-cd: off\n      no-sudo: warn\n";
    let db = db(dockerfile);
    let text = "FROM a:1\nRUN echo hi && \\\n  # why\n  cd /x; make\n";
    assert_eq!(
        found(&db, "Dockerfile", text),
        [("cd-without-exit".into(), "cd /".into())]
    );
    // The body of a heredoc is shell too, for the shell's rules and the Dockerfile's.
    let text = "FROM a:1\nRUN <<EOT\nsudo make\ncd /x\nEOT\n";
    assert_eq!(
        found(&db, "Dockerfile", text),
        [
            ("no-sudo".into(), "RUN ".into()),
            ("cd-without-exit".into(), "cd /".into())
        ]
    );
    // The Dockerfile's suppression comment silences the shell's rule.
    let text = "FROM a:1\n# sudsy-disable-next-line cd-without-exit\nRUN cd /x; make\n";
    assert!(found(&db, "Dockerfile", text).is_empty());
    // The language's own config section applies.
    let db = self::db(&format!(
        "{dockerfile}shell:\n  lint:\n    rules:\n      cd-without-exit: off\n"
    ));
    assert!(found(&db, "Dockerfile", "FROM a:1\nRUN cd /x; make\n").is_empty());
}

#[test]
fn shell_form_cmd_and_backtick_continuations_are_shell() {
    let db = db("dockerfile:\n  lint:\n    rules:\n      use-workdir-not-cd: off\n");
    for text in [
        "FROM a:1\nCMD cd /x; make\n",
        "FROM a:1\nENTRYPOINT cd /x; make\n",
        "FROM a:1\nHEALTHCHECK CMD cd /x; make\n",
        "# escape=`\nFROM a:1\nCMD echo a `\n  && cd /x; make\n",
    ] {
        let rules = found(&db, "Dockerfile", text);
        assert!(
            rules.iter().any(|(r, _)| r == "cd-without-exit"),
            "{text}: {rules:?}"
        );
    }
    // The exec form is not shell; a backtick continuation is not a substitution.
    for text in [
        "FROM a:1\nCMD [\"cd\", \"/x\"]\n",
        "# escape=`\nFROM a:1\nRUN echo a `\n  && echo b `\n  && echo c\n",
    ] {
        let rules = found(&db, "Dockerfile", text);
        assert!(
            rules.iter().all(|(r, _)| r != "cd-without-exit"),
            "{rules:?}"
        );
    }
}

#[test]
fn fences_are_linted_with_their_language() {
    let db = db("");
    let text = "# T\n\n- item\n\n  ```json\n  {\"a\": 1, \"a\": 2}\n  ```\n\n```text\n{\"a\": 1, \"a\": 2}\n```\n";
    let found = found(&db, "a.md", text);
    assert_eq!(
        found,
        [("no-duplicate-keys".into(), "\"a\":".into())],
        "{found:?}"
    );
}

#[test]
fn sql_fences_are_linted_with_the_sql_config() {
    let on = "sql:\n  lint:\n    rules:\n      max-line-length:\n        level: error\n        options:\n          max: 12\n";
    let md = "```sql\nSELECT 1;\nSELECT a, b, c FROM t;\n```\n";
    assert_eq!(
        found(&db(on), "a.md", md),
        [("max-line-length".into(), " c F".into())]
    );
    assert!(found(&db(""), "a.md", md).is_empty());
}

#[test]
fn sql_fences_are_formatted_by_the_sql_formatter() {
    let md = "```sql\nselect a,b from t where x=1\n```\n\n```sql\nSELEC FROM ;;\n```\n";
    let want =
        "```sql\nselect\n    a,\n    b\nfrom t\nwhere x = 1\n```\n\n```sql\nSELEC FROM ;;\n```\n";
    assert_eq!(formatted(&db(""), "a.md", md), want);
}

#[test]
fn fences_are_formatted_by_their_language_unless_off() {
    let md = "- item\n\n  ```json\n  {\"a\":1,\"b\":[1,2]}\n  ```\n\n> ```yaml\n> a:   1\n> ```\n\n```json\n{\"a\":\n```\n\n```ruby\nx   =  1\n```\n";
    let want = "- item\n\n  ```json\n  { \"a\": 1, \"b\": [1, 2] }\n  ```\n\n> ```yaml\n> a: 1\n> ```\n\n```json\n{\"a\":\n```\n\n```ruby\nx   =  1\n```\n";
    let once = formatted(&db(""), "a.md", md);
    assert_eq!(once, want);
    assert_eq!(formatted(&db(""), "a.md", &once), once);
    let off = "markdown:\n  format:\n    embedded_language_formatting: off\n";
    assert_eq!(
        formatted(&db(off), "a.md", md),
        formatted(&db(off), "a.md", &formatted(&db(off), "a.md", md))
    );
    assert!(formatted(&db(off), "a.md", md).contains("{\"a\":1,\"b\":[1,2]}"));
}

#[test]
fn js_ts_and_css_fences_are_formatted_and_linted() {
    let md = "```javascript\nlet a={b:1}\n```\n\n```tsx\nconst x:number=1\n```\n\n```scss\na{b:c}\n```\n";
    let want = "```javascript\nlet a = { b: 1 };\n```\n\n```tsx\nconst x: number = 1;\n```\n\n```scss\na {\n  b: c;\n}\n```\n";
    assert_eq!(formatted(&db(""), "a.md", md), want);
    let on = "js:\n  lint:\n    rules:\n      no-debugger: error\n";
    let linted = found(&db(on), "a.md", "```ts\ndebugger;\n```\n");
    assert_eq!(linted, [("no-debugger".into(), "debu".into())]);
}

#[test]
fn html_script_style_and_attributes_are_formatted_by_their_language_unless_off() {
    let html = "<div style=\"color:red;margin:0\" onclick=\"f( 1,2 )\"></div>
<script>
const a = {b:1}
</script>
<style>
a{color:red}
</style>
<script type=\"text/x\">  a  </script>
<script>if (</script>
";
    let want = "<div style=\"color: red; margin: 0\" onclick=\"f(1, 2)\"></div>
<script>
  const a = { b: 1 };
</script>
<style>
  a {
    color: red;
  }
</style>
<script type=\"text/x\">
  a
</script>
<script>
  if (
</script>
";
    let once = formatted(&db(""), "a.html", html);
    assert_eq!(once, want);
    assert_eq!(formatted(&db(""), "a.html", &once), once);
    let off = "html:
  format:
    embedded_language_formatting: off
";
    assert!(formatted(&db(off), "a.html", html).contains("const a = {b:1}"));
}

#[test]
fn vue_templates_scripts_and_custom_blocks_are_formatted_by_their_language() {
    let mut providers: Vec<Box<dyn LanguageProvider>> = vec![Box::new(lang_html::vue_provider())];
    providers.extend([
        Box::new(lang_js::provider()) as Box<dyn LanguageProvider>,
        Box::new(lang_json::provider()),
    ]);
    let mut db = Database::new(providers);
    db.set_config(ConfigChain::parse(
        "vue:\n  format:\n    indent_script_and_style: false\n",
    ));
    let vue = "<template><Foo :a=\"{b:1}\" @click=\"x=1\" v-for=\"(a,i) in l\">{{a|b}} {{ a+b }}</Foo></template>\n<script>\nlet a={}\n</script>\n<i18n lang=\"json\">{\"a\":1}</i18n>\n";
    let want = "<template>\n  <Foo :a=\"{ b: 1 }\" @click=\"x = 1\" v-for=\"(a, i) in l\"\n    >{{ a | b }} {{ a + b }}</Foo\n  >\n</template>\n<script>\nlet a = {};\n</script>\n<i18n lang=\"json\">\n{ \"a\": 1 }\n</i18n>\n";
    let once = formatted(&db, "a.vue", vue);
    assert_eq!(once, want);
    assert_eq!(formatted(&db, "a.vue", &once), once);
}

#[test]
fn angular_expressions_are_formatted_with_pipes_and_microsyntax() {
    let providers: Vec<Box<dyn LanguageProvider>> = vec![
        Box::new(lang_html::angular_provider()),
        Box::new(lang_js::provider()),
    ];
    let db = Database::new(providers);
    let src = "<div *ngFor=\"let h of hs;trackBy:tb\" [a]=\"x|async\" (c)=\"d( 1 );e=2\">{{n|date:'s'}}</div>\n";
    let once = formatted(&db, "a.component.html", src);
    for want in [
        "*ngFor=\"let h of hs; trackBy: tb\"",
        "[a]=\"x | async\"",
        "(c)=\"d(1); e = 2\"",
        "{{ n | date: \"s\" }}",
    ] {
        assert!(once.contains(want), "{want}: {once}");
    }
    assert_eq!(formatted(&db, "a.component.html", &once), once);
}

#[test]
fn html_fences_are_formatted_and_linted() {
    let md = "```html\n<DIV><p>a</p></DIV>\n```\n";
    assert_eq!(
        formatted(&db(""), "a.md", md),
        "```html\n<div><p>a</p></div>\n```\n"
    );
    let on = "html:\n  lint:\n    rules:\n      deprecated: error\n";
    let linted = found(&db(on), "a.md", "```html\n<center>a</center>\n```\n");
    assert_eq!(linted, [("deprecated".into(), "cent".into())]);
}

#[test]
fn html_front_matter_is_formatted_as_yaml() {
    for (name, provider) in [
        (
            "a.html",
            Box::new(lang_html::provider()) as Box<dyn LanguageProvider>,
        ),
        ("a.vue", Box::new(lang_html::vue_provider())),
        ("a.svelte", Box::new(lang_html::svelte_provider())),
    ] {
        let providers: Vec<Box<dyn LanguageProvider>> =
            vec![provider, Box::new(lang_yaml::provider())];
        let db = Database::new(providers);
        let once = formatted(&db, name, "---\ntitle:   'x'\n---\n<p>a</p>\n");
        assert_eq!(once, "---\ntitle: 'x'\n---\n\n<p>a</p>\n", "{name}");
        assert_eq!(formatted(&db, name, &once), once, "{name}");
    }
}

#[test]
fn svelte_blocks_tags_and_scripts_are_formatted() {
    let providers: Vec<Box<dyn LanguageProvider>> = vec![
        Box::new(lang_html::svelte_provider()),
        Box::new(lang_js::provider()),
        Box::new(lang_css::provider(lang_css::Dialect::Css)),
    ];
    let db = Database::new(providers);
    let svelte = "<style>a{b:c}</style>\n{#if a>1}<p on:click={()=>x++} {y}>{y}</p>{:else}\nno\n{/if}\n<script>\nlet a=1\n</script>\n<input value={value} />\n";
    let want = "<script>\n  let a = 1;\n</script>\n\n{#if a > 1}\n  <p on:click={() => x++} {y}>{y}</p>\n{:else}\n  no\n{/if}\n<input {value} />\n\n<style>\n  a {\n    b: c;\n  }\n</style>\n";
    let once = formatted(&db, "a.svelte", svelte);
    assert_eq!(once, want);
    assert_eq!(formatted(&db, "a.svelte", &once), once);
}

#[test]
fn python_fences_are_formatted_and_linted_with_the_python_config() {
    let md = "```python\nimport os\nx  =  {'a':1}\n```\n\n```py\nx = (\n```\n";
    let want = "```python\nimport os\n\nx = {\"a\": 1}\n```\n\n```py\nx = (\n```\n";
    let once = formatted(&db(""), "a.md", md);
    assert_eq!(once, want);
    assert_eq!(formatted(&db(""), "a.md", &once), once);
    let on = "python:\n  lint:\n    rules:\n      unused-import: error\n";
    // Indented by a list item: the code is parsed without the container prefix.
    let md = "- a\n\n  ```python\n  import os\n  ```\n";
    let linted = found(&db(on), "a.md", md);
    assert_eq!(linted.len(), 1, "{linted:?}");
    assert!(linted[0].1.starts_with("os\n"), "{linted:?}");
}

/// The shell code of `RUN` is laid out by the shell formatter, line breaks are continuations; the
/// options of the `dockerfmt` preset are written out.
#[test]
fn run_shell_is_laid_out_as_continued_lines() {
    let db = db(
        "dockerfile:\n  format:\n    key_value_pairs: true\n    json_copy_as_words: true\n    comment_space: true\n    keep_final_newline: true\n",
    );
    let f = |text: &str| formatted(&db, "Dockerfile", text);
    // a pipeline after `&&` starts level with it, the later `|` lines go one level in
    assert_eq!(
        f("FROM a\nRUN a b > /dev/null \\\n  && f \\\n    | c \\\n    | s \\\n    | x \\\n  && r"),
        "FROM a\nRUN a b > /dev/null \\\n    && f \\\n    | c \\\n        | s \\\n        | x \\\n    && r"
    );
    // control flow keeps its `;`, a `case` arm drops the one before `;;`, a subshell the last
    assert_eq!(
        f(
            "FROM a\nRUN if a; then \\\n  b; \\\n  fi; \\\n  for x in 1 2; do \\\n  echo $x; \\\n  done"
        ),
        "FROM a\nRUN if a; then \\\n        b; \\\n    fi; \\\n    for x in 1 2; do \\\n        echo $x; \\\n    done"
    );
    assert_eq!(
        f("FROM a\nRUN case $x in \\\n  a) \\\n  b; \\\n  ;; \\\n  c) d;; \\\n  esac"),
        "FROM a\nRUN case $x in \\\n        a) \\\n            b \\\n            ;; \\\n        c) d ;; \\\n    esac"
    );
    assert_eq!(
        f("FROM a\nRUN (a; \\\n b;) && \\\n  c"),
        "FROM a\nRUN ( \\\n        a; \\\n        b \\\n    ) \\\n        && c"
    );
    // a comment line stays between the lines, a multi-line quote stays verbatim
    assert_eq!(
        f("FROM a\nRUN a \\\n  # note\n  && b \\\n  && c"),
        "FROM a\nRUN a \\\n    # note\n    && b \\\n    && c"
    );
    assert_eq!(
        f("FROM a\nRUN echo \"x  \\\n   y\" \\\n  && b"),
        "FROM a\nRUN echo \"x  \\\n   y\" \\\n    && b"
    );
    // `RUN \` and the command on the next line joins, unless a comment follows
    assert_eq!(f("FROM a\nRUN \\\n  a && b"), "FROM a\nRUN a && b");
    assert_eq!(
        f("FROM a\nRUN \\\n  # c\n  a"),
        "FROM a\nRUN \\\n    # c\n    a"
    );
    // shell text that does not parse stays as written
    let bad = "FROM a\nRUN Write-Host (a)  b";
    assert_eq!(f(bad), bad);
}

#[test]
fn dockerfile_rewrites_are_options() {
    let text = "FROM a\nENV A b c\nENV P=1 \\\n  Q=2\nLABEL k v\nCOPY [\"a\", \"/d/\"]\n#x\n##y";
    // off by default: only the layout changes
    let off = db("");
    assert_eq!(
        formatted(&off, "Dockerfile", text),
        "FROM a\nENV A b c\nENV P=1 \\\n    Q=2\nLABEL k v\nCOPY [\"a\", \"/d/\"]\n#x\n##y\n"
    );
    let on = db(
        "dockerfile:\n  format:\n    key_value_pairs: true\n    json_copy_as_words: true\n    comment_space: true\n    keep_final_newline: true\n",
    );
    assert_eq!(
        formatted(&on, "Dockerfile", text),
        "FROM a\nENV A=\"b c\"\nENV P=1 \\\n    Q=2\nLABEL k=v\nCOPY a /d/\n# x\n## y"
    );
}
