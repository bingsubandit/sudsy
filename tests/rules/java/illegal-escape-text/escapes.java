class A {
  String a = "\u0009";
//           ^^^^^^^^ illegal-escape-text
  String b = "\011";
//           ^^^^^^ illegal-escape-text
  String c = "\t\u0041";
  char d = '\u0027';
//         ^^^^^^^^ illegal-escape-text
  String e = "\134";
//           ^^^^^^ illegal-escape-text
}
