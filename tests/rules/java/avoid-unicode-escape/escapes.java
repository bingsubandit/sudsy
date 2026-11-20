class A {
  String a = "\u00e9";
//            ^^^^^^ avoid-unicode-escape
  String b = "\u00e9"; // c with acute
  String c = "\u0009\u200b";
  char d = '\u2603';
//          ^^^^^^ avoid-unicode-escape
  String e = "plain";
}
