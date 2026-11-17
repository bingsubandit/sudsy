switch (a) {
  case 1:
    b();
  case 2:
//^ no-fallthrough
    c();
    break;
  case 3:
  case 4:
    d();
    // falls through
  default:
    e();
}
var x = y
//       ^ no-unexpected-multiline
[1].map(z)
var w = y
//       ^ no-unexpected-multiline
(z)
var v = t
//       ^ no-unexpected-multiline
`a`
var r1 = /\x00/, r2 = /[̀a]/, r3 = RegExp("\t"), r4 = /[á]/, r5 = /\n[a-z]/;
//       ^^^^^^ no-control-regex
//                                 ^^^^^^^^^^^^ no-control-regex
//                                                    ^^^^^^ no-misleading-character-class
const mark = "\\u0301";
var r6 = new RegExp("[a" + mark + "]");
//       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-misleading-character-class
