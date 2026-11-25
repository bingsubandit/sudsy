/**
 * @file Make grammar for tree-sitter
 * @author Alexandre Muller
 * @author Amaan Qureshi <amaanq12@gmail.com>
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const CHARSET = 'a-zA-Z0-9%\\+\\-\\.@_\\*\\?\\/!\\[\\]';
// the characters of a word that may also start an assignment operator (`+=`, `?=`, `!=`): a word does not end in one
// (`A+=b` is the word `A` and `+=`); a run of them at its end is a `_word_tail`
const OPCHARS = '\\+\\?!';
const PLAINSET = 'a-zA-Z0-9%\\-\\.@_\\*\\/\\[\\]';
const ESCAPE_SET = 'abtnvfrE!"#\\$&\'\\(\\)\\*,;<>\\?\\[\\\\\\]^`{\\|}~';

const NL = token.immediate(/[\r\n]+/);
const WS = token.immediate(/[\t ]+/);
const SPLIT = alias(token.immediate(seq('\\', /\r?\n|\r/)), '\\');

const AUTOMATIC_VARS = ['@', '%', '<', '?', '^', '+', '/', '*'];

const DEFINE_OPS = ['=', ':=', '::=', '?=', '+='];

const FUNCTIONS = [
  'subst',
  'patsubst',
  'strip',
  'findstring',
  'filter',
  'filter-out',
  'sort',
  'word',
  'words',
  'wordlist',
  'firstword',
  'lastword',
  'dir',
  'notdir',
  'suffix',
  'basename',
  'addsuffix',
  'addprefix',
  'join',
  'wildcard',
  'realpath',
  'abspath',
  'error',
  'warning',
  'info',
  'origin',
  'flavor',
  'foreach',
  'if',
  'or',
  'and',
  'call',
  'eval',
  'file',
  'value',
];

module.exports = grammar({
  name: 'make',

  word: $ => $.word,

  inline: $ => [
    $._targets,
    $._target_pattern,
    $._prerequisites_pattern,
    $._prerequisites,
    $._order_only_prerequisites,
    $._target_or_pattern_assignment,

    $._primary,
    $._name,
    $._string,
  ],

  extras: $ => [
    /\s/,
    alias(token(seq('\\', /\r?\n|\r/)), '\\'),
    $.comment,
  ],

  rules: {
    // 3.1
    makefile: $ => repeat($._thing),

    _thing: $ => choice(
      $.rule,
      $._variable_definition,
      $._directive,
      seq($._function, NL),
      alias($._stray_paren_line, $.function_call),
      alias($._reference_line, $.function_call),
    ),

    // Rules {{{
    // 2.1
    rule: $ => choice(
      $._ordinary_rule,
      $._static_pattern_rule,
    ),

    _ordinary_rule: $ => prec.right(seq(
      $._targets,
      choice(':', '&:', '::'),
      optional(WS),
      optional($._prerequisites),
      choice(
        $.recipe,
        NL,
      ),
    )),

    // 4.12.1
    _static_pattern_rule: $ => prec.right(seq(
      $._targets,
      ':',
      optional(WS),
      $._target_pattern,
      ':',
      optional(WS),
      optional(choice(
        $._prerequisites_pattern,
        seq(optional($._prerequisites_pattern), '|', $._order_only_prerequisites),
      )),
      choice(
        $.recipe,
        NL,
      ),
    )),

    // `$(error x))`: make takes the stray parenthesis as text
    _stray_paren_line: $ => seq($._function, repeat1(alias(')', $.word)), NL),

    // a statement that is a reference (`$(recurse)`): make expands it to makefile text
    _reference_line: $ => seq($._variable, NL),

    _targets: $ => alias($.list, $.targets),

    // LINT: List shall have length one
    _target_pattern: $ => field(
      'target',
      alias($.list, $.pattern_list),
    ),

    // 4.3
    _prerequisites: $ => choice(
      $._normal_prerequisites,
      seq(
        optional($._normal_prerequisites),
        '|',
        $._order_only_prerequisites,
      ),
    ),

    _normal_prerequisites: $ => field(
      'normal',
      alias($.list, $.prerequisites),
    ),

    _order_only_prerequisites: $ => field(
      'order_only',
      alias($.list, $.prerequisites),
    ),

    _prerequisites_pattern: $ => field(
      'prerequisite',
      alias($.list, $.pattern_list),
    ),

    recipe: $ => prec.right(choice(
      // the first recipe line may be attached to the
      // target-and-prerequisites line with a semicolon
      // in between
      seq(
        $._attached_recipe_line,
        NL,
        repeat(choice(
          $.conditional,
          $._prefixed_recipe_line,
        )),
      ),
      seq(
        NL,
        repeat1(choice(
          $.conditional,
          $._prefixed_recipe_line,
        )),
      ),
    )),

    _attached_recipe_line: $ => seq(
      ';',
      optional($.recipe_line),
    ),

    _prefixed_recipe_line: $ => seq(
      $._recipeprefix,
      optional($.recipe_line),
      NL,
    ),

    recipe_line: $ => choice(
      // a command that is only its prefix (`\t@`)
      choice(...['@', '-', '+'].map(c => token(prec(1, c)))),
      seq(
        optional(choice(
          ...['@', '-', '+'].map(c => token(prec(1, c))),
        )),
        optional(seq(
          alias($.shell_text_with_split, $.shell_text),
          repeat(seq(
            // splited recipe lines may start with .RECIPEPREFIX
            // that shall not be part of the shell_code
            optional($._recipeprefix),
            alias($.shell_text_with_split, $.shell_text),
          )),
          optional($._recipeprefix),
        )),
        alias($._shell_text_without_split, $.shell_text),
      ),
    ),
    // }}}
    // Variables {{{
    _variable_definition: $ => choice(
      $.VPATH_assignment,
      $.RECIPEPREFIX_assignment,
      $.variable_assignment,
      $.shell_assignment,
      $.define_directive,
    ),

    // 4.5.1
    VPATH_assignment: $ => seq(
      field('name', 'VPATH'),
      optional(WS),
      field('operator', choice(...DEFINE_OPS)),
      field('value', $.paths),
      NL,
    ),

    RECIPEPREFIX_assignment: $ => seq(
      field('name', '.RECIPEPREFIX'),
      optional(WS),
      field('operator', choice(...DEFINE_OPS)),
      field('value', $.text),
      NL,
    ),

    // 6.5
    variable_assignment: $ => seq(
      optional(seq($._target_or_pattern_assignment, optional(choice('export', 'override', 'private')))),
      $._name,
      optional(WS),
      field('operator', choice(...DEFINE_OPS)),
      optional(WS),
      optional(field('value', alias($._value_text, $.text))),
      NL,
    ),

    _target_or_pattern_assignment: $ => seq(
      field('target_or_pattern', $.list),
      ':',
      optional(WS),
    ),

    shell_assignment: $ => seq(
      field('name', $.word),
      optional(WS),
      field('operator', '!='),
      optional(WS),
      field('value', $._shell_command),
      NL,
    ),

    define_directive: $ => seq(
      'define',
      $._name,
      optional(WS),
      optional(field('operator', choice(...DEFINE_OPS))),
      optional(WS),
      NL,
      optional(field('value',
        alias(repeat1(choice($._rawline, $._nested_define)), $.raw_text),
      )),
      token(prec(1, 'endef')),
      NL,
    ),
    // }}}
    // Directives {{{
    _directive: $ => choice(
      $.include_directive,
      $.vpath_directive,
      $.export_directive,
      $.unexport_directive,
      $.override_directive,
      $.undefine_directive,
      $.private_directive,
      $.conditional,
    ),

    // 3.3
    include_directive: $ => choice(
      seq('include', field('filenames', $.list), NL),
      seq('sinclude', field('filenames', $.list), NL),
      seq('-include', field('filenames', $.list), NL),
    ),

    // 4.5.2
    vpath_directive: $ => choice(
      seq('vpath', NL),
      seq('vpath', field('pattern', $.word), NL),
      seq('vpath', field('pattern', $.word), field('directories', $.paths), NL),
    ),

    // 5.7.2
    export_directive: $ => choice(
      seq('export', NL),
      seq('export', field('variables', $.list), NL),
      seq('export', $.variable_assignment),
    ),

    // 5.7.2
    unexport_directive: $ => choice(
      seq('unexport', NL),
      seq('unexport', field('variables', $.list), NL),
    ),

    // 6.7
    override_directive: $ => choice(
      seq('override', $.define_directive),
      seq('override', $.variable_assignment),
      seq('override', $.undefine_directive),
    ),

    // 6.9
    undefine_directive: $ => seq(
      'undefine', field('variable', $.word), NL,
    ),

    // 6.13
    private_directive: $ => seq(
      'private', $.variable_assignment,
    ),
    // }}}
    // Conditionals {{{
    // 7
    conditional: $ => seq(
      field('condition', $._conditional_directives),
      optional(field('consequence', $._conditional_consequence)),
      repeat($.elsif_directive),
      optional($.else_directive),
      'endif',
      NL,
    ),

    elsif_directive: $ => seq(
      'else',
      optional(WS),
      field('condition', $._conditional_directives),
      optional(field('consequence', $._conditional_consequence)),
    ),

    else_directive: $ => seq(
      'else',
      optional(WS),
      NL,
      optional(field('consequence', $._conditional_consequence)),
    ),

    _conditional_directives: $ => choice(
      $.ifeq_directive,
      $.ifneq_directive,
      $.ifdef_directive,
      $.ifndef_directive,
    ),

    _conditional_consequence: $ => repeat1(choice(
      $._thing,
      $._prefixed_recipe_line,
    )),

    ifeq_directive: $ => seq(
      'ifeq', $._conditional_args_cmp, NL,
    ),

    ifneq_directive: $ => seq(
      'ifneq', $._conditional_args_cmp, NL,
    ),

    ifdef_directive: $ => seq(
      'ifdef', field('variable', $._primary), NL,
    ),

    ifndef_directive: $ => seq(
      'ifndef', field('variable', $._primary), NL,
    ),

    _conditional_args_cmp: $ => choice(
      seq(
        '(',
        optional(field('arg0', $._cond_arg)),
        ',',
        optional(field('arg1', $._cond_arg)),
        ')',
      ),
      seq(
        field('arg0', $._primary),
        field('arg1', $._primary),
      ),
    ),

    // }}}
    // Variables {{{
    _variable: $ => choice(
      $.variable_reference,
      $.substitution_reference,
      $.automatic_variable,
    ),

    variable_reference: $ => seq(
      '$',
      choice(
        delimitedVariable($._primary),
        // `$()`: the empty variable, a common way to write a blank (`$(subst $() $(),:,x)`)
        seq(token.immediate('('), ')'),
        // TODO are those legal? $) $$$
        alias(token.immediate(/./), $.word), // match any single digit
        // `$\` and a newline: the empty variable that joins two lines without a blank
        alias(token.immediate(prec(1, /\\\r?\n|\\\r/)), $.word),
      ),
    ),

    // a pattern or replacement; a hidden rule of its own, so that its states are not copied into each reference context
    _subarg: $ => choice($._primary, alias($._braced_word, $.word)),

    // 6.3.1
    substitution_reference: $ => seq(
      '$',
      delimitedVariable($._subst_body),
    ),

    _subst_body: $ => seq(
        // $(@:%.o=%.c): the automatic variable's token would otherwise not be a word here
        field('text', choice(
          $._primary,
          alias(choice(...AUTOMATIC_VARS.map(c => token(prec(1, c)))), $.word),
        )),
        ':',
        optional(field('pattern', $._subarg)),
        '=',
        optional(field('replacement', $._subarg)),
    ),

    // 10.5.3
    automatic_variable: _ => seq(
      '$',
      choice(
        choice(
          ...AUTOMATIC_VARS
            .map(c => token.immediate(prec(1, c))),
        ),
        delimitedVariable(seq(
          choice(
            ...AUTOMATIC_VARS
              .map(c => token(prec(1, c))),
          ),
          optional(choice(
            token.immediate('D'),
            token.immediate('F'),
          )),
        )),
      ),
    ),
    // }}}
    // Functions {{{
    _function: $ => choice(
      $.function_call,
      $.shell_function,
    ),

    function_call: $ => seq(
      '$',
      token.immediate('('),
      // the blank is part of the name token so that a variable named like a function ($(dir)) is a reference
      field('function', choice(
        ...FUNCTIONS.map(f => token.immediate(seq(f, /[	 ]/))),
      )),
      optional(WS),
      optional($.arguments),
      ')',
    ),

    // an argument may be empty ($(if $(a),b,))
    arguments: $ => choice(
      field('argument', $.text),
      seq(
        optional(field('argument', $.text)),
        repeat1(seq(',', optional(field('argument', $.text)))),
      ),
    ),

    // 8.13
    shell_function: $ => seq(
      '$',
      token.immediate('('),
      field('function', 'shell'),
      optional(WS),
      $._shell_command,
      ')',
    ),
    // }}}
    // Primary and lists {{{
    list: $ => prec(1, seq(
      $._primary,
      repeat(seq(
        choice(WS, SPLIT),
        $._primary,
      )),
      optional(WS),
    )),

    paths: $ => seq(
      $._primary,
      repeat(seq(
        choice(...[':', ';'].map(c => token.immediate(c))),
        $._primary,
      )),
    ),

    _primary: $ => choice(
      $.word,
      alias($._word_tail, $.word),
      $.archive,
      $._variable,
      $._function,
      $.concatenation,
      $.string,
    ),

    concatenation: $ => prec.right(seq(
      $._primary,
      repeat1(prec.left($._primary)),
    )),
    // }}}
    // Names {{{
    _name: $ => field('name', choice($.word, $.concatenation, $._variable)),

    string: $ => field('string', choice(
      seq('"', optional($._string), '"'),
      seq('\'', optional($._string), '\''),
    )),

    _string: $ => repeat1(choice(
      $._variable,
      $._function,
      token(prec(-1, /([^'"$\r\n\\]|\\\\|\\[^\r\n])+/)),
    )),

    word: _ => {
      const plain = choice(
        new RegExp('[' + PLAINSET + ']'),
        new RegExp('\\\\[' + ESCAPE_SET + ']'),
        new RegExp('\\\\[0-9]{3}'),
      );
      const ops = new RegExp('[' + OPCHARS + ']');
      return token(seq(repeat(ops), plain, repeat(choice(plain, seq(repeat1(ops), plain)))));
    },

    // `c++`, `a?`: the operator characters at the end of a word, right after it
    _word_tail: _ => token.immediate(new RegExp('[' + OPCHARS + ']+')),

    // an argument of ifeq and friends: a primary, or a word that also has `;`, `:`, `=` and `'` (`ifeq ($(a), b='c';)`)
    _cond_arg: $ => choice($._primary, alias($._cond_word, $.word)),

    _cond_word: _ => token(repeat1(choice(
      new RegExp('[' + CHARSET + ';:=\'\\\\]'),
      new RegExp('\\\\[' + ESCAPE_SET + ']'),
    ))),

    // a word with braces or commas, as the pattern and replacement of a substitution reference have (`$(c:%=a{b})`, `$(c:%=-Wl,%)`)
    _braced_word: _ => token(seq(
      repeat(choice(new RegExp('[' + CHARSET + ']'), new RegExp('\\\\[' + ESCAPE_SET + ']'))),
      new RegExp('[{},]'),
      repeat(choice(new RegExp('[' + CHARSET + '{},]'), new RegExp('\\\\[' + ESCAPE_SET + ']'))),
    )),

    // 11.1
    archive: $ => seq(
      field('archive', $.word),
      token.immediate('('),
      field('members', $.list),
      token.immediate(')'),
    ),
    // }}}
    // Tokens {{{
    // TODO external parser for .RECIPEPREFIX
    _recipeprefix: _ => '\t',

    // a `define ... endef` inside the body of a define: make counts the nesting
    _nested_define: $ => seq(
      token(prec(2, /[ \t]*define[ \t][^\r\n]*[\r\n]+/)),
      repeat(choice($._rawline, $._nested_define)),
      token(prec(2, /[ \t]*endef[^\r\n]*[\r\n]+/)),
    ),

    // TODO prefixed line in define is recipe
    _rawline: _ => token(/.*[\r\n]+/), // any line

    _shell_text_without_split: $ => text(
      noneOf(...['\\$', '\\r', '\\n', '\\']),
      choice(
        $._variable,
        $._function,
        alias('$$', $.escape),
        alias('//', $.escape),
      ),
      // a `#` in a command is shell text, even right after a reference
      token.immediate(prec(2, /#[^$\r\n\\]*/)),
    ),

    // The SPLIT chars shall be included the injected code
    shell_text_with_split: $ => seq(
      $._shell_text_without_split,
      SPLIT,
    ),

    _shell_command: $ => alias(
      $.text,
      $.shell_command,
    ),

    // balanced parentheses in a value or an argument: SOURCES = ( \ ... ), goal(s)
    _paren_group: $ => seq('(', optional($.text), ')'),

    // the value of an assignment: parentheses need not balance (`A = case x in 0) ...`)
    _value_text: $ => text(
      choice(
        noneOf(...['\\$', '\\n', '\\r', '\\']),
        SPLIT,
      ),
      choice(
        $._variable,
        $._function,
        alias('$$', $.escape),
        alias('//', $.escape),
      ),
    ),

    text: $ => text(
      choice(
        noneOf(...['\\$', '\\(', '\\)', '\\n', '\\r', '\\']),
        SPLIT,
      ),
      choice(
        $._variable,
        $._function,
        $._paren_group,
        alias('$$', $.escape),
        alias('//', $.escape),
      ),
    ),
    // }}}

    comment: _ => token(prec(-1, /#.*/)),
  },
});

/**
 *
 * @param {...any} characters
 */
function noneOf(...characters) {
  const negatedString = characters.map(c => c == '\\' ? '\\\\' : c).join('');
  return new RegExp('[^' + negatedString + ']');
}

/**
 *
 * @param {RuleOrLiteral} rule
 */
function delimitedVariable(rule) {
  return choice(
    seq(token.immediate('('), rule, ')'),
    seq(token.immediate('{'), rule, '}'),
  );
}

/**
 *
 * @param {RuleOrLiteral} text
 * @param {RuleOrLiteral} fenced_vars
 */
function text(text, fenced_vars, after_reference = null) {
  const raw_text = token(repeat1(choice(
    text,
    new RegExp('\\\\[' + ESCAPE_SET + ']'),
    new RegExp('\\\\[0-9]{3}'),
    new RegExp('\\\\[^\n\r]'), // used in cmd like sed \1
  )));
  const after = after_reference ? choice(raw_text, after_reference) : raw_text;
  return choice(
    seq(
      raw_text,
      repeat(seq(
        fenced_vars,
        optional(after),
      )),
    ),
    seq(
      fenced_vars,
      repeat(seq(
        optional(after),
        fenced_vars,
      )),
      optional(after),
    ),
  );
}
