//! Statements, declarations and literals against Prettier's output (each expectation was checked
//! with Prettier 3 under its defaults), with every check of the shared helper.

use core_config::OptionValue;
use core_format::testing::assert_formats;
use provider_api::format_testing::Formatting;

fn fmt(input: &str) -> String {
    assert_formats(&Formatting::new(lang_js::provider(), "a.ts"), input)
}

fn fmt_with(options: Vec<(&'static str, OptionValue)>, input: &str) -> String {
    let mut subject = Formatting::new(lang_js::provider(), "a.ts");
    subject.options = options;
    assert_formats(&subject, input)
}

#[test]
fn statements_and_blank_lines() {
    assert_eq!(
        fmt(
            "'use strict'\nlet a=1,b\nconst c=2,d=3\n\n\n\nif(a)b();else if(c){d()}else e()\nfor(let i=0;i < n;i++)f(i)\nfor(;;){}\nfor(const k in o)g(k)\nasync function q(){for await(const x of y){}}\nwhile(1)x()\ndo{y()}while(z)\nlbl:for(;;)break lbl\n"
        ),
        "\"use strict\";\nlet a = 1,\n  b;\nconst c = 2,\n  d = 3;\n\nif (a) b();\nelse if (c) {\n  d();\n} else e();\nfor (let i = 0; i < n; i++) f(i);\nfor (;;) {}\nfor (const k in o) g(k);\nasync function q() {\n  for await (const x of y) {\n  }\n}\nwhile (1) x();\ndo {\n  y();\n} while (z);\nlbl: for (;;) break lbl;\n"
    );
}

#[test]
fn switch_try_and_jumps() {
    assert_eq!(
        fmt(
            "switch(x){case 1:case 2:a();break\n\ndefault:{b()}}\ntry{f()}catch(e){g()}finally{h()}\ntry{}catch{}\nthrow new Error('x')\nfunction f(){return}\ndebugger\n"
        ),
        "switch (x) {\n  case 1:\n  case 2:\n    a();\n    break;\n\n  default: {\n    b();\n  }\n}\ntry {\n  f();\n} catch (e) {\n  g();\n} finally {\n  h();\n}\ntry {\n} catch {}\nthrow new Error(\"x\");\nfunction f() {\n  return;\n}\ndebugger;\n"
    );
}

#[test]
fn functions_classes_and_modules() {
    assert_eq!(
        fmt(
            "export default async function*  f<T>(a: T, b: number = 1, ...r): void {}\nfunction g(this: X, { a, b }){}\nexport class A<T> extends B implements C,D{\nstatic x?: number = 1\n\n\n  async *m(a){}\nget [k](){return 1}\n@dec() p;constructor(private a: string){super()}\nabstract n(): void\nstatic{init()}\n}\nimport d,{a as b,type c} from 'x' with { type: 'json' }\nimport {} from 'y'\nimport 'z'\nexport {a,b as c}\nexport * as ns from \"z\"\n"
        ),
        "export default async function* f<T>(a: T, b: number = 1, ...r): void {}\nfunction g(this: X, { a, b }) {}\nexport class A<T> extends B implements C, D {\n  static x?: number = 1;\n\n  async *m(a) {}\n  get [k]() {\n    return 1;\n  }\n  @dec() p;\n  constructor(private a: string) {\n    super();\n  }\n  abstract n(): void;\n  static {\n    init();\n  }\n}\nimport d, { a as b, type c } from \"x\" with { type: \"json\" };\nimport {} from \"y\";\nimport \"z\";\nexport { a, b as c };\nexport * as ns from \"z\";\n"
    );
}

#[test]
fn long_parameters_and_import_lists_break() {
    assert_eq!(
        fmt(
            "function longName(parameterNumberOne, parameterNumberTwo, parameterNumberThree, four) {}\nimport {aaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb, cccccccccccccccccc, ddddddddddddddddd} from 'x'\n"
        ),
        "function longName(\n  parameterNumberOne,\n  parameterNumberTwo,\n  parameterNumberThree,\n  four,\n) {}\nimport {\n  aaaaaaaaaaaaaaaa,\n  bbbbbbbbbbbbbbbbbbb,\n  cccccccccccccccccc,\n  ddddddddddddddddd,\n} from \"x\";\n"
    );
}

#[test]
fn literals_are_restyled_inside_unformatted_expressions() {
    assert_eq!(
        fmt("f('a', \"it's\", 'say \"hi\"', 0XAB, 1.50, .5, 5., 1E+05, /x/gi, /y/ig, 10n)\n"),
        "f(\"a\", \"it's\", 'say \"hi\"', 0xab, 1.5, 0.5, 5, 1e5, /x/gi, /y/gi, 10n);\n"
    );
}

#[test]
fn comments_stay_with_their_tokens() {
    assert_eq!(
        fmt(
            "// top\nlet a = 1; // one\n/* two */ if (a) {\n  // three\n  b(); // four\n}\n// last\n"
        ),
        "// top\nlet a = 1; // one\n/* two */ if (a) {\n  // three\n  b(); // four\n}\n// last\n"
    );
}

#[test]
fn comments_before_a_semicolon_go_after_it_and_parameters_keep_blank_lines() {
    assert_eq!(
        fmt(
            "a = 2 /* c */;\nlet x = 1 /* d */;\nfunction f(\n  a,\n\n  b,\n) {}\n// top\n(a && b) && c;\n"
        ),
        "a = 2; /* c */\nlet x = 1; /* d */\nfunction f(\n  a,\n\n  b,\n) {}\n// top\na && b && c;\n"
    );
}

#[test]
fn parentheses_and_quotes_of_the_expressions_and_members() {
    assert_eq!(
        fmt(
            "(a?.b)?.c;\nexport default (function () {}.x());\n(a ?? b) ? c : d;\nclass K { \"m\"() {} }\nimport(\"./a/very/long/module/path/to/a/very/long/module/path/to/a/very/long/module/path\");\n"
        ),
        "a?.b?.c;\nexport default (function () {}.x());\n(a ?? b) ? c : d;\nclass K {\n  m() {}\n}\nimport(\"./a/very/long/module/path/to/a/very/long/module/path/to/a/very/long/module/path\");\n"
    );
}

#[test]
fn the_semi_and_quote_options() {
    let semi_off = vec![("semi", OptionValue::Bool(false))];
    assert_eq!(
        fmt_with(
            semi_off,
            "const a = 1;\n[1].map(f);\n(g)();\nclass A { x = 1; [k] = 2; y; *m() {} }\nfor (;;) {}\n"
        ),
        "const a = 1\n;[1].map(f)\ng()\nclass A {\n  x = 1;\n  [k] = 2\n  y;\n  *m() {}\n}\nfor (;;) {}\n"
    );
    let single = vec![("quote_style", OptionValue::String("single".into()))];
    assert_eq!(
        fmt_with(
            single,
            "'use strict'\nf(\"a\", 'b', \"it's\", \"say \\\"hi\\\"\")\nimport x from \"x\"\n"
        ),
        "'use strict';\nf('a', 'b', \"it's\", 'say \"hi\"');\nimport x from 'x';\n"
    );
}

#[test]
fn empty_statements_in_a_list_go() {
    assert_eq!(
        fmt("function f() {};\nif (a) {\n  ;b.c()\n  ;\n} else ;\nfor (;;) ;\n"),
        "function f() {}\nif (a) {\n  b.c();\n} else;\nfor (;;);\n"
    );
}

#[test]
fn class_semicolons_and_specifier_blank_lines() {
    assert_eq!(
        fmt("class A { m() {}; x = 1; }\nexport {\n  // a\n  a,\n\n  // b\n  b,\n}\n"),
        "class A {\n  m() {}\n  x = 1;\n}\nexport {\n  // a\n  a,\n\n  // b\n  b,\n};\n"
    );
}

#[test]
fn prettier_ignore_keeps_a_statement() {
    assert_eq!(
        fmt("// prettier-ignore\nlet   a  =  'x'\nlet   b  =  'y'\n"),
        "// prettier-ignore\nlet   a  =  'x'\nlet b = \"y\";\n"
    );
}

#[test]
fn typescript_types_and_declarations() {
    assert_eq!(
        fmt(
            r#"type A = "aaaaaaaaaaaaaaaaaaaaaaa" | "bbbbbbbbbbbbbbbbbbbbbbbbbbb" | "cccccccccccccccccccccc" | "dddddddddd";
type B<T> = (T extends string ? "a" : T extends number ? "b" : "cccccccccccccccccccccccccccccccccccccccccccc");
type C = { readonly [K in keyof T as `get${K}`]-?: T[K] } & { a: string, b?: number };
interface D<T extends object = {}> extends E, F { a: string; readonly b?: number, [key: string]: any; m<U>(x: U): void; new (x: number): D<T> }
enum G { A = 1, B, C = "c" }
declare module "m" { export function f(a: (string | number)[], b: Array<(() => void)>): void; }
namespace H.I { export const x = 1 }
abstract class J<T> extends K<T> implements L, M { abstract m(): void; constructor(private a: string, readonly b = 1) { super() } }
function f<T extends keyof U, U>(this: Window, a: T, ...rest: U[]): asserts a is T {}
let v = x as unknown as B, w = <T>y!, z = a satisfies B;
type N = [a: string, b?: number, ...rest: boolean[]];
type O = (aaaaaaaaaaaaaaaaaaaaaaa: string, bbbbbbbbbbbbbbbbbbbbbbbbbbb: number, ccccccccccccccc: boolean) => void;
"#
        ),
        r#"type A =
  | "aaaaaaaaaaaaaaaaaaaaaaa"
  | "bbbbbbbbbbbbbbbbbbbbbbbbbbb"
  | "cccccccccccccccccccccc"
  | "dddddddddd";
type B<T> = T extends string
  ? "a"
  : T extends number
    ? "b"
    : "cccccccccccccccccccccccccccccccccccccccccccc";
type C = { readonly [K in keyof T as `get${K}`]-?: T[K] } & {
  a: string;
  b?: number;
};
interface D<T extends object = {}> extends E, F {
  a: string;
  readonly b?: number;
  [key: string]: any;
  m<U>(x: U): void;
  new (x: number): D<T>;
}
enum G {
  A = 1,
  B,
  C = "c",
}
declare module "m" {
  export function f(a: (string | number)[], b: Array<() => void>): void;
}
namespace H.I {
  export const x = 1;
}
abstract class J<T> extends K<T> implements L, M {
  abstract m(): void;
  constructor(
    private a: string,
    readonly b = 1,
  ) {
    super();
  }
}
function f<T extends keyof U, U>(
  this: Window,
  a: T,
  ...rest: U[]
): asserts a is T {}
let v = x as unknown as B,
  w = <T>y!,
  z = a satisfies B;
type N = [a: string, b?: number, ...rest: boolean[]];
type O = (
  aaaaaaaaaaaaaaaaaaaaaaa: string,
  bbbbbbbbbbbbbbbbbbbbbbbbbbb: number,
  ccccccccccccccc: boolean,
) => void;
"#
    );
}

#[test]
fn objects_arrays_and_patterns() {
    // Expected output checked with Prettier 3 under its defaults.
    assert_eq!(
        fmt(
            "const {a,b:c=1,...r}=o;\nconst [x,,y=2]=arr;\nconst o1={a,'b':1,[c]:2,'d-e':3,...e,m(){}};\nconst o2={\n  a:1};\nconst {a:{b}}=obj;\nconst nums=[1,2,3];\nconst objs=[{a:1,b:2},{a:3,b:4}];\nfoo({a:1,b:2},function(){});\n"
        ),
        "const { a, b: c = 1, ...r } = o;\nconst [x, , y = 2] = arr;\nconst o1 = { a, b: 1, [c]: 2, \"d-e\": 3, ...e, m() {} };\nconst o2 = {\n  a: 1,\n};\nconst {\n  a: { b },\n} = obj;\nconst nums = [1, 2, 3];\nconst objs = [\n  { a: 1, b: 2 },\n  { a: 3, b: 4 },\n];\nfoo({ a: 1, b: 2 }, function () {});\n"
    );
}

#[test]
fn object_options() {
    let options = vec![
        ("quote_props", OptionValue::String("consistent".into())),
        ("trailing_commas", OptionValue::String("none".into())),
        ("bracket_spacing", OptionValue::Bool(false)),
        ("arrow_parens", OptionValue::String("avoid".into())),
    ];
    assert_eq!(
        fmt_with(
            options,
            "a={x:1,'y-z':2};\nb={'x':1,y:2};\nf=(x)=>x;\ng=async(x)=>x;\nh=({x})=>x;\nk=[aaaaaaaaaaaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,cccccccccccccccccccccccc];\n"
        ),
        "a = {\"x\": 1, \"y-z\": 2};\nb = {x: 1, y: 2};\nf = x => x;\ng = async x => x;\nh = ({x}) => x;\nk = [\n  aaaaaaaaaaaaaaaaaaaaaaaaaaaa,\n  bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,\n  cccccccccccccccccccccccc\n];\n"
    );
}

#[test]
fn comment_placement_matches_prettier() {
    // Each expectation was checked with Prettier 3 under its defaults.
    assert_eq!(
        fmt(
            "function a() {\n  return (\n    // c\n    x\n  );\n}\nif (a) {\n  b;\n} // t\nelse {\n  c;\n}\nfunction f() { // o\n  d();\n}\nfor (;;) {\n  continue /* c */;\n}\nclass C {\n  m() {} /* D */\n}\nconst fn = (/* e */) => 1;\n"
        ),
        "function a() {\n  return (\n    // c\n    x\n  );\n}\nif (a) {\n  b;\n} // t\nelse {\n  c;\n}\nfunction f() {\n  // o\n  d();\n}\nfor (;;) {\n  continue; /* c */\n}\nclass C {\n  m() {} /* D */\n}\nconst fn = (/* e */) => 1;\n"
    );
    assert_eq!(
        fmt(
            "interface I {\n  a: X | undefined // else\n  b: Y\n}\ntype Q<\n  A, // first\n  B\n> = A;\n"
        ),
        "interface I {\n  a: X | undefined; // else\n  b: Y;\n}\ntype Q<\n  A, // first\n  B,\n> = A;\n"
    );
}

#[test]
fn ignore_and_asi_without_semicolons() {
    assert_eq!(
        fmt("type V = {\n  // prettier-ignore\n  a:   string;\n  b:   number;\n};\n"),
        "type V = {\n  // prettier-ignore\n  a:   string;\n  b: number;\n};\n"
    );
    let semi_off = vec![("semi", OptionValue::Bool(false))];
    assert_eq!(
        fmt_with(
            semi_off,
            "interface I { a: 1; b(): void }\nif (a) (b)()\nelse [c].map(d)\nx\n++y\n"
        ),
        "interface I {\n  a: 1\n  b(): void\n}\nif (a) b()\nelse [c].map(d)\nx\n++y\n"
    );
}

#[test]
fn comments_of_commas_and_stacked_line_comments_are_kept() {
    assert_eq!(
        fmt(
            "export { a, // one\n  b, // two\n};\nconst x = [1,\n  // hole\n  ,\n  2];\nf(a, // c1\n  // c2\n  b);\n"
        ),
        "export {\n  a, // one\n  b, // two\n};\nconst x = [\n  1,\n  // hole\n  ,\n  2,\n];\nf(\n  a, // c1\n  // c2\n  b,\n);\n"
    );
}

#[test]
fn glued_comments_jsdoc_templates_and_unions() {
    // Each expectation was checked with Prettier 3 under its defaults.
    let u = "Aaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbb | Cccccccccccccccc | Dddddddddddddddd";
    assert_eq!(
        fmt(&format!(
            "f(/* a */ b, /* c */ d);
g(/* e */ );
class A {{
	/**
	 * Doc
	 */
	m() {{}}
}}
x = `a ${{ b( c ) }} d`;
type T = {u};
"
        )),
        format!(
            "f(/* a */ b, /* c */ d);
g(/* e */);
class A {{
  /**
   * Doc
   */
  m() {{}}
}}
x = `a ${{b(c)}} d`;
type T =
  {u};
"
        )
    );
}

#[test]
fn comments_between_head_and_body_and_in_empty_lists() {
    // Each expectation was checked with Prettier 3 under its defaults.
    assert_eq!(
        fmt("if (a) // c
{
  b;
}
interface I { // d
  x: number;
}
const u = [
  // none
];
if (!(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb && cccccccccccccc)) {}
"),
        "if (a) {
  // c
  b;
}
interface I {
  // d
  x: number;
}
const u = [
  // none
];
if (!(
  aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &&
  bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &&
  cccccccccccccc
)) {
}
"
    );
}

#[test]
fn casts_this_parameters_logical_operands_and_pattern_types() {
    // Each expectation was checked with Prettier 3 under its defaults.
    assert_eq!(
        fmt(
            "(a as B) += 1;\nx = a as B === c;\ny = (a && b) && (c && d);\ntype F = (this: T, n: number) => R;\ninterface I { m<T>(this: A, b: B): void }\nfunction g({ cert, key }: { cert: CryptoFile; key: CryptoFile; kkkkkkkkkkkkkkkkkkkkkkk: string }) {}\n"
        ),
        "(a as B) += 1;\nx = (a as B) === c;\ny = a && b && c && d;\ntype F = (this: T, n: number) => R;\ninterface I {\n  m<T>(this: A, b: B): void;\n}\nfunction g({\n  cert,\n  key,\n}: {\n  cert: CryptoFile;\n  key: CryptoFile;\n  kkkkkkkkkkkkkkkkkkkkkkk: string;\n}) {}\n"
    );
}

#[test]
fn header_comments_move_into_the_body() {
    // A stable placement (Prettier keeps them in the header); the check formats the output again.
    assert_eq!(
        fmt(
            "class C2 // a\nextends Base // b\n{\n  m() {}\n}\ninterface A2 // c\nextends B {\n  x: number;\n}\n"
        ),
        "class C2 extends Base {\n  // a\n  // b\n  m() {}\n}\ninterface A2 extends B {\n  // c\n  x: number;\n}\n"
    );
}

#[test]
fn comments_around_dropped_tokens_reach_a_fixed_point() {
    // Each input changed on a second format before: parentheses and bars that go, `{` after a
    // block comment, `switch` heads, a comment before the `;` of a loop body, a comma before `)`.
    assert_eq!(
        fmt(
            "(a = c /* c */);\nif (1) /* b */ // c\n{x}\nswitch (t) // s\n{}\nfor (;;) x() // d\n;\nlet u: /*1*/ | /*2*/ C | D;\nf(a, /* e */);\nm = /*p*/ ( Fn(() => {}).once() )().v;\n"
        ),
        "a = c; /* c */\nif (1) /* b */ {\n  // c\n  x;\n}\nswitch (t) { // s\n}\nfor (;;) x(); // d\n\nlet u: /*1*/ /*2*/ C | D;\nf(a /* e */);\nm = /*p*/ Fn(() => {}).once()().v;\n"
    );
}

#[test]
fn inputs_the_safety_check_refused_before() {
    // Each was refused as an internal error: a line break after `return` (ASI), a comment lost
    // after `for (;;)`, parentheses that `let`, `type ... as`, `new (import())` and `in` need.
    assert_eq!(
        fmt(
            "function f() {\n  return (\n    // c\n    a\n  )()\n}\nfor (;;) /* b */ ;\nfor ((let) of x);\n(let)[a] = 1;\n(type) as unknown;\nnew (import(\"a\"))();\nfor (var a = (b in c) in {});\n"
        ),
        "function f() {\n  return (\n    // c\n    a()\n  );\n}\nfor (;;) /* b */;\nfor ((let) of x);\n(let)[a] = 1;\n(type) as unknown;\nnew (import(\"a\"))();\nfor (var a = (b in c) in {});\n"
    );
    assert_eq!(
        fmt(
            "measure() // w\n  .then(() => 1);\ninterface I {\n  // prettier-ignore\n  x: y,\n}\nconst s = `a\r\nb`;\n"
        ),
        "measure() // w\n  .then(() => 1);\ninterface I {\n  // prettier-ignore\n  x: y,\n}\nconst s = `a\nb`;\n"
    );
}

#[test]
fn comments_beside_dropped_parentheses_reach_a_fixed_point() {
    // Each of these changed on a second format.
    assert_eq!(
        fmt(
            "(a = c /* c */);\nfunction f() { return /* b */ ; }\nvoid (a && b // c\n);\n!+(3 /* foo */);\n"
        ),
        "a = c; /* c */\nfunction f() {\n  return; /* b */\n}\nvoid (\n  a &&\n  b // c\n);\n!+(3 /* foo */);\n"
    );
    assert_eq!(
        fmt(
            "x = [1, // a\n  2];\nfoo</* c */ T>();\ntype U =\n  | A // a\n  | B;\nconst g = // c\n  (a) => (b) => c;\n"
        ),
        "x = [\n  1, // a\n  2,\n];\nfoo</* c */ T>();\ntype U =\n  | A // a\n  | B;\nconst g = // c\n  (a) => (b) => c;\n"
    );
}

#[test]
fn line_comments_in_a_cast_chain_reach_a_fixed_point() {
    // A line comment inside the operand of a cast does not break the parentheses around it; a
    // second comment after a line comment goes on its own line at the indentation of the next line.
    assert_eq!(
        fmt(
            "f = (x // a\n  .z as P).y;\ng = ((x // a\n  .l as P) // b\n  .e as P) // c\n  .e; // d\n"
        ),
        "f = (x.z as P).y; // a\ng = ((x.l as P).e as P).e; // a\n// b\n// c\n// d\n"
    );
}

#[test]
fn with_optional_calls_and_long_strings_reach_a_fixed_point() {
    assert_eq!(
        fmt("with (a /* c */) b;\nx = a /* c */ ?.(b);\ny = f('a\"b');\n"),
        "with (a /* c */) b;\nx = a /* c */?.(b);\ny = f('a\"b');\n"
    );
}

#[test]
fn chains_and_parameters_that_hugged_at_a_second_format_reach_a_fixed_point() {
    // An expanded chain of two calls keeps its layout when only the object's source line break
    // would make the one-line form fit.
    let chain = "const f = (cb) =>\n  textureNode\n    .isolate()\n    .context({\n      getUV: (t) => callback(t.uvNode || uv()),\n      forceUVContext: true,\n    });\n";
    assert_eq!(fmt(chain), chain);
    // A comment after a lone object parameter keeps the parameter from hugging the parentheses.
    assert_eq!(
        fmt(
            "class A {\n  m({ lightDirection, lightColor, reflectedLight, lightDirection2, lightColor2, reflectedLight2 }, /* b */ ) {}\n}\n"
        ),
        "class A {\n  m(\n    {\n      lightDirection,\n      lightColor,\n      reflectedLight,\n      lightDirection2,\n      lightColor2,\n      reflectedLight2,\n    },\n  /* b */) {}\n}\n"
    );
    // Parentheses around a non-null expression are not a chain head of their own.
    assert_eq!(
        fmt(
            "x = (\n    glimseGlyphsHazardNoopsTieTie\n    ?. averredBathersBoxroomBuggyNurl\n    ?.anodyneCondosMalateOverateRetinol!\n  )\n  ?.annularCooeedSplicesWalksWayWay\n  .annularCooeedSplicesWalksWayWay(annularCooeedSplicesWalksWayWay)\n  .annularCooeedSplicesWalksWayWay();\n"
        ),
        "x = glimseGlyphsHazardNoopsTieTie?.averredBathersBoxroomBuggyNurl\n  ?.anodyneCondosMalateOverateRetinol!?.annularCooeedSplicesWalksWayWay.annularCooeedSplicesWalksWayWay(\n    annularCooeedSplicesWalksWayWay,\n  )\n  .annularCooeedSplicesWalksWayWay();\n"
    );
}

#[test]
fn a_template_body_is_not_on_its_own_line_for_trivia_after_it() {
    // (the trailing line break of the file is not a line break inside the template)
    assert_eq!(
        fmt(
            "const f = (level) => `calc(var(--spacing) * ${(level - 1) * config.perLevel + cfg.base})`\n"
        ),
        "const f = (level) =>\n  `calc(var(--spacing) * ${(level - 1) * config.perLevel + cfg.base})`;\n"
    );
}

#[test]
fn line_comments_after_operators_and_optional_chain_bodies() {
    // Expected: Prettier 3 output.
    assert_eq!(
        fmt(
            "type A<T> =\n  T extends X\n    ? // yes\n      Y\n    : // no\n      Z;\nconst k = // c\n  f(a);\nlet o // c\n= {a};\nxs.find((x) =>\n  someLongName?.includesTheValueOfSomething(xxxxxxxxxxxxxxxxxxxxxxxxx),\n);\n"
        ),
        "type A<T> = T extends X\n  ? // yes\n    Y\n  : // no\n    Z;\nconst k = // c\n  f(a);\nlet o = // c\n  { a };\nxs.find((x) =>\n  someLongName?.includesTheValueOfSomething(xxxxxxxxxxxxxxxxxxxxxxxxx),\n);\n"
    );
}
