//! The shape of a workflow: its keys, kinds, event names, activity types and permission scopes,
//! as rows read by the shared `core_rules::shape` walk.
//!
//! concept: key-shape

use core_rules::rule;
use core_rules::shape::{Combos, Format, Kind, Row, Scoped, Shape, Typed, Value, Variant, check};
use core_rules::{FileContext, RuleContext};

const S: &str = "workflow-syntax";
const E: &str = "invalid-event";
const FILTER: &str =
    "\"{key}\" filter is not available for {parent} event. it is only for {list} events";
const BRANCHES: &[&str] = &[
    "merge_group",
    "push",
    "pull_request",
    "pull_request_target",
    "workflow_run",
];
const PATHS: &[&str] = &["push", "pull_request", "pull_request_target"];
const P: &str = "invalid-permissions";
const O: &str = "outdated-action";
const G: &str = "invalid-glob";

#[rustfmt::skip]
const EVENTS: &[&str] = &[
    "branch_protection_rule", "check_run", "check_suite", "create", "delete", "deployment",
    "deployment_status", "discussion", "discussion_comment", "fork", "gollum", "issue_comment",
    "issues", "label", "merge_group", "milestone", "page_build", "project", "project_card",
    "project_column", "public", "pull_request", "pull_request_review", "pull_request_review_comment",
    "pull_request_target", "push", "registry_package", "release", "repository_dispatch", "schedule",
    "status", "watch", "workflow_call", "workflow_dispatch", "workflow_run", "image_version",
];
#[rustfmt::skip]
const SCOPES: &[&str] = &[
    "actions", "attestations", "checks", "contents", "deployments", "discussions", "id-token",
    "issues", "models", "packages", "pages", "pull-requests", "repository-projects",
    "security-events", "statuses",
];
#[rustfmt::skip]
const PR_TYPES: &[&str] = &[
    "assigned", "unassigned", "labeled", "unlabeled", "opened", "edited", "closed", "reopened",
    "synchronize", "converted_to_draft", "locked", "unlocked", "enqueued", "dequeued", "milestoned",
    "demilestoned", "ready_for_review", "review_requested", "review_request_removed",
    "auto_merge_enabled", "auto_merge_disabled",
];
/// The specs of popular actions whose runner is retired (actionlint's list of its pinned release).
const TIMEZONES: &str = include_str!("../data/timezones.txt");
const OUTDATED: &str = include_str!("../data/outdated-actions.txt");
static MATRIX: Combos = Combos {
    include: "include",
    exclude: "exclude",
};
static DISPATCH_INPUT: Typed = Typed {
    type_key: "type",
    default_key: "default",
    options_key: "options",
    required_key: "required",
    choice: "choice",
    types: &[("boolean", Value::Bool), ("number", Value::Number)],
    required_no_default: false,
};
static CALL_INPUT: Typed = Typed {
    type_key: "type",
    default_key: "default",
    options_key: "options",
    required_key: "required",
    choice: "",
    types: &[("boolean", Value::Bool), ("number", Value::Number)],
    required_no_default: true,
};
const FILTER_PAIRS: &[(&str, &str)] = &[
    ("branches", "branches-ignore"),
    ("tags", "tags-ignore"),
    ("paths", "paths-ignore"),
];

macro_rules! row {
    ($path:expr, $rule:expr, $kind:expr $(, $field:ident: $value:expr)* $(,)?) => {
        Row { path: $path, rule: $rule, kind: $kind $(, $field: $value)*, ..Row::NONE }
    };
}

#[rustfmt::skip]
static SHAPE: Shape = Shape {
    variants: &[
        Variant { name: "uses", has: &["uses"], lacks: &[] },
        Variant { name: "run", has: &["run"], lacks: &[] },
        Variant { name: "no-runs", has: &[], lacks: &["runs"] },
    ],
    expression: "${{",
    structure: S,
    scoped: &[
        Scoped { key: "branches", within: BRANCHES, rule: E, message: FILTER },
        Scoped { key: "branches-ignore", within: BRANCHES, rule: E, message: FILTER },
        Scoped { key: "paths", within: PATHS, rule: E, message: FILTER },
        Scoped { key: "paths-ignore", within: PATHS, rule: E, message: FILTER },
        Scoped { key: "tags", within: &["push"], rule: E, message: FILTER },
        Scoped { key: "tags-ignore", within: &["push"], rule: E, message: FILTER },
    ],
    rows: &[
        row!("", S, Kind::Map, title: "workflow", variant: "no-runs", required: &["on", "jobs"],
            keys: &["name", "run-name", "on", "permissions", "env", "defaults", "concurrency", "jobs"]),
        row!("on", E, Kind::Any, names: EVENTS, keys: EVENTS, non_empty: true,
            mapped: &["schedule"]),
        row!("on.push", E, Kind::Map, exclusive: FILTER_PAIRS,
            keys: &["branches", "branches-ignore", "tags", "tags-ignore", "paths", "paths-ignore"]),
        row!("on.pull_request|on.pull_request_target", E, Kind::Map, exclusive: FILTER_PAIRS,
            keys: &["types", "branches", "branches-ignore", "paths", "paths-ignore"]),
        row!("on.pull_request.types|on.pull_request_target.types", E, Kind::Any, names: PR_TYPES, non_empty: true),
        row!("on.workflow_run", E, Kind::Map, keys: &["types", "workflows", "branches", "branches-ignore"],
            exclusive: &[("branches", "branches-ignore")]),
        row!("on.workflow_run.types", E, Kind::Any, names: &["completed", "requested", "in_progress"], non_empty: true),
        row!("on.workflow_run.workflows|on.workflow_run.branches|on.workflow_run.branches-ignore\
              |on.push.branches|on.push.branches-ignore|on.push.tags|on.push.tags-ignore|on.push.paths\
              |on.push.paths-ignore|on.pull_request.branches|on.pull_request.branches-ignore\
              |on.pull_request.paths|on.pull_request.paths-ignore|on.pull_request_target.branches\
              |on.pull_request_target.branches-ignore|on.pull_request_target.paths\
              |on.pull_request_target.paths-ignore", E, Kind::Any, non_empty: true),
        row!("on.issues.types", E, Kind::Any, non_empty: true, names: &["opened", "edited", "deleted",
            "transferred", "pinned", "unpinned", "closed", "reopened", "assigned", "unassigned", "labeled",
            "unlabeled", "locked", "unlocked", "milestoned", "demilestoned"]),
        row!("on.issue_comment.types|on.pull_request_review_comment.types|on.discussion_comment.types|on.label.types",
            E, Kind::Any, names: &["created", "edited", "deleted"], non_empty: true),
        row!("on.release.types", E, Kind::Any, non_empty: true, names: &["published", "unpublished", "created",
            "edited", "deleted", "prereleased", "released"]),
        row!("on.check_run.types", E, Kind::Any, non_empty: true,
            names: &["created", "rerequested", "completed", "requested_action"]),
        row!("on.check_suite.types", E, Kind::Any, names: &["completed"], non_empty: true),
        row!("on.pull_request_review.types", E, Kind::Any, names: &["submitted", "edited", "dismissed"], non_empty: true),
        row!("on.milestone.types", E, Kind::Any, non_empty: true,
            names: &["created", "closed", "opened", "edited", "deleted"]),
        row!("on.watch.types", E, Kind::Any, names: &["started"], non_empty: true),
        row!("on.merge_group.types", E, Kind::Any, names: &["checks_requested"], non_empty: true),
        row!("on.registry_package.types", E, Kind::Any, names: &["published", "updated"], non_empty: true),
        row!("on.branch_protection_rule|on.check_run|on.check_suite|on.discussion|on.discussion_comment\
              |on.issue_comment|on.issues|on.label|on.milestone|on.project|on.project_card\
              |on.project_column|on.pull_request_review|on.pull_request_review_comment|on.registry_package\
              |on.release|on.watch|on.repository_dispatch", E, Kind::Map, keys: &["types"]),
        row!("on.merge_group", E, Kind::Map, keys: &["types", "branches", "branches-ignore"], exclusive: FILTER_PAIRS),
        row!("on.schedule", E, Kind::Seq, non_empty: true),
        row!("on.schedule.*", E, Kind::Map, keys: &["cron", "timezone"], required: &["cron"]),
        row!("on.workflow_dispatch.inputs", E, Kind::Map, typed: Some(&DISPATCH_INPUT), unique: true),
        row!("on.workflow_call.inputs", E, Kind::Map, typed: Some(&CALL_INPUT), unique: true),
        row!("on.*.branches.*|on.*.branches-ignore.*|on.*.tags.*|on.*.tags-ignore.*|on.image_version.versions.*",
            G, Kind::Str, format: Format::RefGlob),
        row!("on.*.paths.*|on.*.paths-ignore.*", G, Kind::Str, format: Format::PathGlob),
        row!("jobs.*.steps.*.id", S, Kind::Str, format: Format::Identifier),
        row!("jobs.*.container.credentials.password|jobs.*.services.*.credentials.password", S, Kind::Str,
            format: Format::Expression),
        row!("jobs.*.steps.*.uses", O, Kind::Str, format: Format::Outdated(OUTDATED)),
        row!("on.schedule.*.cron", E, Kind::Str, format: Format::Cron, non_empty: true),
        row!("on.schedule.*.timezone", E, Kind::Str, non_empty: true, format: Format::Listed(TIMEZONES,
            "invalid timezone \"{text}\" in schedule event. it must be a valid IANA timezone name")),
        row!("on.image_version", E, Kind::Map, keys: &["names", "versions"]),
        row!("on.image_version.names|on.image_version.versions", E, Kind::Seq, non_empty: true),
        row!("on.image_version.names.*|on.image_version.versions.*", E, Kind::Str, non_empty: true),
        row!("on.workflow_dispatch", E, Kind::Map, keys: &["inputs"]),
        row!("on.workflow_dispatch.inputs|on.workflow_call.inputs|on.workflow_call.outputs\
              |on.workflow_call.secrets", E, Kind::Map, unique: true),
        row!("on.workflow_dispatch.inputs.*", E, Kind::Map,
            keys: &["description", "required", "default", "type", "options"]),
        row!("on.workflow_dispatch.inputs.*.type", E, Kind::Str, non_empty: true,
            names: &["string", "number", "boolean", "environment", "choice"]),
        row!("on.workflow_dispatch.inputs.*.options", E, Kind::Seq, non_empty: true),
        row!("on.workflow_dispatch.inputs.*.options.*", E, Kind::Str, non_empty: true),
        row!("on.*.types", E, Kind::Any, non_empty: true),
        row!("on.workflow_call", E, Kind::Map, keys: &["inputs", "outputs", "secrets"]),
        row!("on.workflow_call.inputs.*", E, Kind::Map,
            keys: &["description", "required", "default", "type"], required: &["type"]),
        row!("on.workflow_call.inputs.*.type", E, Kind::Str, names: &["string", "number", "boolean"], non_empty: true),
        row!("on.workflow_call.outputs.*", E, Kind::Map, keys: &["description", "value"], required: &["value"]),
        row!("on.workflow_call.outputs.*.value", E, Kind::Str, non_empty: true),
        row!("on.workflow_call.secrets.*", E, Kind::Map, keys: &["description", "required"]),
        row!("permissions|jobs.*.permissions", P, Kind::Any, names: &["read-all", "write-all"], keys: SCOPES,
            non_empty: true, empty_map_ok: true),
        row!("permissions.id-token|jobs.*.permissions.id-token", P, Kind::Str, names: &["write", "none"]),
        row!("permissions.models|jobs.*.permissions.models", P, Kind::Str, names: &["read", "none"]),
        row!("permissions.*|jobs.*.permissions.*", P, Kind::Str, names: &["read", "write", "none"]),
        row!("env|jobs.*.env|jobs.*.steps.*.env|jobs.*.container.env|jobs.*.services.*.env", S, Kind::Map,
            unique: true, non_empty: true),
        row!("env.*|jobs.*.env.*|jobs.*.steps.*.env.*|jobs.*.container.env.*|jobs.*.services.*.env.*",
            S, Kind::Scalar),
        row!("defaults|jobs.*.defaults", S, Kind::Map, keys: &["run"], required: &["run"], non_empty: true),
        row!("defaults.run|jobs.*.defaults.run", S, Kind::Map, keys: &["shell", "working-directory"]),
        row!("concurrency|jobs.*.concurrency", S, Kind::Any, keys: &["group", "cancel-in-progress"],
            required: &["group"], non_empty: true),
        row!("jobs", S, Kind::Map, non_empty: true),
        row!("jobs.*", S, Kind::Map, title: "reusable workflow call job", variant: "uses",
            keys: &["name", "uses", "with", "secrets", "needs", "if", "permissions", "strategy", "concurrency"]),
        row!("jobs.*.uses", S, Kind::Str, non_empty: true, format: Format::Pattern(
            r"^(?:\./[^@\s]+|[^/@\s.][^/@\s]*/[^/@\s]+/[^@\s]+@\S+)$|\$\{\{",
            "reusable workflow call \"{text}\" at \"uses\" is not following the format \"owner/repo/path/to/workflow.yml@ref\" nor \"./path/to/workflow.yml\". see https://docs.github.com/en/actions/learn-github-actions/reusing-workflows#calling-a-reusable-workflow")),
        row!("jobs.*", S, Kind::Map, title: "job", required: &["runs-on", "steps"],
            keys: &["name", "needs", "runs-on", "permissions", "environment", "concurrency", "outputs", "env",
                "defaults", "if", "steps", "timeout-minutes", "strategy", "continue-on-error", "container",
                "services", "snapshot"]),
        row!("jobs.*.needs", S, Kind::Any, non_empty: true),
        row!("jobs.*.environment", S, Kind::Any, keys: &["name", "url", "deployment"], required: &["name"],
            non_empty: true),
        row!("jobs.*.snapshot", S, Kind::Any, keys: &["image-name", "version", "if"], required: &["image-name"],
            non_empty: true),
        row!("jobs.*.snapshot.image-name|jobs.*.snapshot.version", S, Kind::Str, non_empty: true),
        row!("jobs.*.runs-on", S, Kind::Any, non_empty: true, keys: &["group", "labels"]),
        row!("jobs.*.runs-on.labels", S, Kind::Any, non_empty: true),
        row!("jobs.*.runs-on.labels.*", S, Kind::Str, non_empty: true),
        row!("jobs.*.runs-on.*|jobs.*.runs-on.group", S, Kind::Str, non_empty: true),
        row!("jobs.*.continue-on-error|jobs.*.steps.*.continue-on-error|jobs.*.strategy.fail-fast", S, Kind::Bool),
        row!("jobs.*.timeout-minutes|jobs.*.steps.*.timeout-minutes", S, Kind::Float, format: Format::Positive),
        row!("jobs.*.strategy.max-parallel", S, Kind::Int, format: Format::Positive),
        row!("jobs.*.strategy", S, Kind::Map, keys: &["matrix", "fail-fast", "max-parallel"], non_empty: true),
        row!("jobs.*.strategy.matrix", S, Kind::Any, unique: true, combos: Some(&MATRIX)),
        row!("jobs.*.strategy.matrix.*", S, Kind::Seq, title: "matrix values", non_empty: true),
        row!("jobs.*.outputs", S, Kind::Map, non_empty: true),
        row!("jobs.*.outputs.*", S, Kind::Scalar),
        row!("jobs.*.container|jobs.*.services.*", S, Kind::Any, required: &["image"], non_empty: true, null_is_text: true,
            keys: &["image", "credentials", "env", "ports", "volumes", "options"]),
        row!("jobs.*.services", S, Kind::Map, unique: true, non_empty: true),
        row!("jobs.*.container.image|jobs.*.services.*.image|jobs.*.container.options\
              |jobs.*.services.*.options", S, Kind::Str, non_empty: true),
        row!("jobs.*.container.ports|jobs.*.services.*.ports|jobs.*.container.volumes|jobs.*.services.*.volumes",
            S, Kind::Seq, non_empty: true),
        row!("jobs.*.container.ports.*|jobs.*.services.*.ports.*|jobs.*.container.volumes.*|jobs.*.services.*.volumes.*",
            S, Kind::Str, non_empty: true),
        row!("jobs.*.container.credentials|jobs.*.services.*.credentials", S, Kind::Map, non_empty: true,
            keys: &["username", "password"], required: &["username", "password"], together: true),
        row!("jobs.*.steps", S, Kind::Seq, non_empty: true),
        row!("jobs.*.steps.*", S, Kind::Map, title: "step to execute action", variant: "uses",
            non_empty: true,
            keys: &["id", "if", "name", "env", "continue-on-error", "timeout-minutes", "uses", "with"]),
        row!("jobs.*.steps.*", S, Kind::Map, title: "step to run shell command", variant: "run",
            non_empty: true,
            keys: &["id", "if", "name", "env", "continue-on-error", "timeout-minutes", "run", "shell", "working-directory"]),
        row!("jobs.*.steps.*", S, Kind::Map, title: "step", non_empty: true, required: &["run"]),
        row!("jobs.*.steps.*.with|jobs.*.with|jobs.*.secrets", S, Kind::Map, unique: true, non_empty: true),
        row!("jobs.*.steps.*.with.*|jobs.*.with.*", S, Kind::Scalar),
    ],
};

macro_rules! shape_rule {
    ($name:ident, $id:expr, $description:expr, $why:expr, $example:expr) => {
        rule! {
            pub $name {
                id: $id,
                category: Correctness,
                default: Off,
                description: $description,
                why: $why,
                example: $example,
            }

            fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
                check(&SHAPE, $id, file, ctx);
            }
        }
    };
}

shape_rule!(
    WorkflowSyntax,
    "workflow-syntax",
    "Reports a key that a section does not have, a required key or section that is \
     missing, a section of the wrong kind (a list where a map is needed), or an empty \
     one.",
    "The mistake shows only after the workflow is pushed, when it does not start or does not do what the file seems to say.\n\
     To fix, correct the key, or add what is missing.",
    "on: push\njobs:\n  a:\n    runs-on: ubuntu-latest\n    stepz: []   # reported"
);
shape_rule!(
    InvalidEvent,
    "invalid-event",
    "Reports an event name, activity type, filter or schedule that the `on` section \
     cannot have.",
    "An event or activity type that GitHub does not have never fires. A misspelled \
     `pushh`, or a type that the event does not have, means the workflow does not run \
     when its author expects.\n\
     To fix, use the event, type and filter that the documentation lists.",
    "on:\n  pushh: {}   # reported\njobs: {}"
);
shape_rule!(
    InvalidGlob,
    "invalid-glob",
    "Reports a branch, tag or path filter that is not a valid pattern: a bad range or \
     escape, a character that a ref name cannot have, or `.` or `..` in a path.",
    "A pattern that GitHub cannot read does not select the refs or paths it was written \
     for, so the workflow runs for the wrong pushes or not at all.\n\
     To fix, correct the pattern.",
    "on:\n  push:\n    tags: ['v*+']   # reported\njobs: {}"
);
shape_rule!(
    OutdatedAction,
    "outdated-action",
    "Reports a step that uses a popular action at a version whose runner (the Node \
     version it runs on) is retired, from a list of actions kept with the rule.",
    "GitHub retires the Node runtimes that old action versions use. Their jobs print \
     deprecation warnings and, once the runtime is removed, fail.\n\
     To fix, update the action to a newer version.",
    "on: push\njobs:\n  a:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v2   # reported"
);
shape_rule!(
    InvalidPermissions,
    "invalid-permissions",
    "Reports a permission scope or value that GitHub does not have.",
    "A misspelling like `content: read` does not grant the access that the job needs, and a job that sets `permissions` has no access to the scopes it does not name.\n\
     To fix, use a known scope with a value it takes (`read`, `write` or `none`).",
    "on: push\npermissions:\n  content: read   # reported\njobs: {}"
);
