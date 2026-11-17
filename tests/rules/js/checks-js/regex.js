const a = /\x1f/;
//        ^^^^^^ no-control-regex
const b = new RegExp("\\u0001");
//        ^^^^^^^^^^^^^^^^^^^^^ no-control-regex
const c = /[a\u0301]/;
//        ^^^^^^^^^^^ no-misleading-character-class
const d = /[a-z]/;
