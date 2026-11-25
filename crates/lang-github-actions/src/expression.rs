//! The `${{ }}` expressions of a workflow: the contexts and their types, the functions, and the
//! keys where each context is available, as tables read by the shared `core_rules::expr`.
//!
//! concept: expression-language

use core_rules::expr::{Func, Place, Rules, Scope, ScopeKind, Spec, check};
use core_rules::rule;
use core_rules::{FileContext, RuleContext};
use std::sync::OnceLock;

const OBJECT: &[&str] = &["{*: any}"];
const JOB: &[&str] = &["github", "needs", "strategy", "matrix", "vars", "inputs"];
const STEP: &[&str] = &[
    "github", "needs", "strategy", "matrix", "job", "runner", "env", "vars", "secrets", "steps",
    "inputs",
];

macro_rules! place {
    ($path:expr, $contexts:expr $(, $field:ident: $value:expr)*) => {
        Place { path: $path, contexts: $contexts $(, $field: $value)*, ..Place::NONE }
    };
}

macro_rules! func {
    ($name:expr, $params:expr, $min:expr, $ret:expr $(, $field:ident: $value:expr)*) => {
        Func { name: $name, params: $params, min: $min, ret: $ret $(, $field: $value)*, ..Func::NONE }
    };
}

#[rustfmt::skip]
#[allow(clippy::needless_update)]
static SPEC: Spec = Spec {
    open: "${{",
    close: "}}",
    contexts: &[
        ("github", "{action: string; action_path: string; action_ref: string; action_repository: string; action_status: string; actor: string; actor_id: string; api_url: string; base_ref: string; env: string; event: {*: any}; event_name: string; event_path: string; graphql_url: string; head_ref: string; job: string; job_workflow_sha: string; path: string; ref: string; ref_name: string; ref_protected: any; ref_type: string; repository: string; repository_id: string; repository_owner: string; repository_owner_id: string; repositoryurl: string; retention_days: string; run_attempt: string; run_id: string; run_number: string; secret_source: string; server_url: string; sha: string; token: string; triggering_actor: string; workflow: string; workflow_ref: string; workflow_sha: string; workspace: string}"),
        ("env", "{*: string}"),
        ("vars", "{*: string}"),
        ("secrets", "{*: string}"),
        ("job", "{container: {id: string; network: string}; services: {*: {id: string; network: string; ports: {*: string}}}; status: string; check_run_id: number}"),
        ("runner", "{name: string; os: string; arch: string; temp: string; tool_cache: string; debug: string; environment: string}"),
        ("strategy", "{fail-fast: bool; job-index: number; job-total: number; max-parallel: number}"),
        ("steps", "any"),
        ("needs", "any"),
        ("matrix", "any"),
        ("inputs", "any"),
        ("jobs", "any"),
    ],
    functions: &[
        func!("contains", &["array<any>|string", "any"], 2, "bool"),
        func!("startsWith", &["string", "string"], 2, "bool"),
        func!("endsWith", &["string", "string"], 2, "bool"),
        func!("format", &["string", "any"], 1, "string", variadic: true, format: true),
        func!("join", &["array<any>|string", "string"], 1, "string"),
        func!("toJSON", &["any"], 1, "string"),
        func!("fromJSON", &["string"], 1, "any", json: true),
        func!("hashFiles", &["string"], 1, "string", variadic: true, special: true),
        func!("success", &[], 0, "bool", special: true),
        func!("always", &[], 0, "bool", special: true),
        func!("cancelled", &[], 0, "bool", special: true),
        func!("failure", &[], 0, "bool", special: true),
    ],
    places: &[
        place!("run-name|concurrency", &["github", "inputs", "vars"]),
        place!("env", &["github", "secrets", "inputs", "vars"], accepts: OBJECT, expects: "object"),
        place!("defaults", &[]),
        place!("on.workflow_call.inputs.*.default", &["github", "inputs", "vars"], typed_by: "type"),
        place!("on.workflow_call.outputs.*.value", &["github", "jobs", "vars", "inputs"]),
        place!("jobs.*.if", &["github", "needs", "vars", "inputs"],
            specials: &["always", "cancelled", "success", "failure"], implicit: true),
        place!("jobs.*.steps.*.if", &["github", "needs", "strategy", "matrix", "job", "runner", "env", "vars", "steps", "inputs"],
            specials: &["always", "cancelled", "success", "failure", "hashFiles"], implicit: true),
        place!("jobs.*.steps.*.id|jobs.*.steps.*.uses|jobs.*.steps.*.shell", &[]),
        place!("jobs.*.steps.*.env", STEP, specials: &["hashFiles"], accepts: OBJECT, expects: "object"),
        place!("jobs.*.steps", STEP, specials: &["hashFiles"]),
        place!("jobs.*.strategy", &["github", "needs", "vars", "inputs"]),
        place!("jobs.*.container.credentials|jobs.*.services.*.credentials",
            &["github", "needs", "strategy", "matrix", "vars", "inputs", "env", "secrets"]),
        place!("jobs.*.container.env|jobs.*.services.*.env",
            &["github", "needs", "strategy", "matrix", "vars", "inputs", "job", "runner", "env", "secrets"]),
        place!("jobs.*.defaults", &["github", "needs", "strategy", "matrix", "vars", "inputs", "env"]),
        place!("jobs.*.env", &["github", "needs", "strategy", "matrix", "vars", "inputs", "secrets"], accepts: OBJECT, expects: "object"),
        place!("jobs.*.services", JOB, accepts: OBJECT, expects: "object"),
        place!("jobs.*.runs-on", JOB, accepts: &["string", "array<any>"], expects: "string or array"),
        place!("jobs.*.environment.url", &["github", "needs", "strategy", "matrix", "vars", "inputs", "job", "runner", "env", "steps"]),
        place!("jobs.*.outputs", &["github", "needs", "strategy", "matrix", "vars", "inputs", "job", "runner", "env", "secrets", "steps"]),
        place!("jobs.*.secrets", &["github", "needs", "strategy", "matrix", "secrets", "inputs", "vars"]),
        place!("jobs.*.with", &["github", "needs", "strategy", "matrix", "inputs", "vars"]),
        place!("jobs.*", JOB),
    ],
    scopes: &[
        Scope { context: "steps", kind: ScopeKind::Ids, within: "jobs.*", source: "steps", key: "id",
            item: "{outputs: {*: string}; conclusion: string; outcome: string}", ..Scope::NONE },
        Scope { context: "needs", kind: ScopeKind::Refs, within: "jobs.*", source: "needs", from: "jobs",
            output: "outputs", item: "{result: string}", open_with: "uses", ..Scope::NONE },
        Scope { context: "jobs", kind: ScopeKind::Refs, from: "jobs", output: "outputs", item: "{result: string}", ..Scope::NONE },
        Scope { context: "matrix", within: "jobs.*", source: "strategy.matrix", skip: &["include", "exclude"],
            also: "include", item: "infer", ..Scope::NONE },
        Scope { context: "secrets", source: "on.workflow_call.secrets", item: "string",
            base: &["github_token", "actions_runner_debug", "actions_step_debug"], ..Scope::NONE },
        Scope { context: "inputs", source: "on.workflow_dispatch.inputs|on.workflow_call.inputs", key: "type",
            item: "string", types: &[("boolean", "bool"), ("number", "number")], strict: true, ..Scope::NONE },
        Scope { context: "github.event.inputs", source: "on.workflow_dispatch.inputs", item: "string", ..Scope::NONE },
    ],
    rules: Rules { syntax: "invalid-expression", context: "expression-context", types: "expression-types", condition: "if-condition" },
    compiled: OnceLock::new(),
};

macro_rules! expr_rule {
    ($name:ident, $id:expr, $default:ident, $description:expr, $why:expr, $example:expr) => {
        rule! {
            pub $name {
                id: $id,
                category: Correctness,
                default: $default,
                description: $description,
                why: $why,
                example: $example,
            }

            fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
                check(&SPEC, $id, file, ctx);
            }
        }
    };
}

expr_rule!(
    InvalidExpression,
    "invalid-expression",
    Error,
    "Reports a `${{ }}` expression that is not closed or is empty, that does not parse \
     (unbalanced brackets, a string that is not closed, a character expressions do not \
     have, such as a double quote), or that names a context or function that does not \
     exist.",
    "The mistake shows only after the workflow is pushed, when the run fails.\n\
     To fix, correct the expression.",
    "name: ${{ github.sha }\nenv:\n  A: ${{ foo.bar }}   # reported"
);
expr_rule!(
    ExpressionContext,
    "expression-context",
    Off,
    "Reports an expression that reads a context, or calls a special function, that the \
     key it is in does not offer (`env` in the `env` of a job, `runner` in a matrix).",
    "Each key of a workflow can use only some contexts, since the others do not exist \
     yet when it is evaluated. An expression that reads another one does not get its value.\n\
     To fix, use a context the key offers, or move the value to a key that has it.",
    "jobs:\n  a:\n    runs-on: ubuntu-latest\n    env:\n      A: ${{ env.B }}   # reported"
);
expr_rule!(
    IfCondition,
    "if-condition",
    Off,
    "Reports an `if:` that is a constant (`if: false`), or one with text around its `${{ \
     }}`, which makes it a non-empty string that is always true (`if: ${{ a }} && ${{ b \
     }}`).",
    "A string with text around the expression is never false, even when the expression \
     is `false`, so the step or job always runs. A constant condition is left in by \
     mistake after a test.\n\
     To fix, write the whole condition inside one `${{ }}`, or without it.",
    "on: push\njobs:\n  a:\n    runs-on: ubuntu-latest\n    if: ${{ github.ref }} && true   # reported"
);
expr_rule!(
    ExpressionTypes,
    "expression-types",
    Off,
    "Reports an expression whose types do not fit: a property that an object does not \
     have, a comparison of values that cannot be compared, a wrong number or type of \
     arguments, or an object where text is expected.",
    "Expressions fail without an error: a property that does not exist is an empty \
     string, and an object where text is expected becomes the text `Object`. The \
     workflow runs with a wrong value.\n\
     To fix, correct the property, or the types.",
    "on: push\nrun-name: ${{ github.events }}   # reported"
);
