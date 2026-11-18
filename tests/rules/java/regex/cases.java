class A {
  void f(String s) {
    Pattern.compile("\u001f");
//  ^^^^^^^^^^^^^^^^^^^^^^^^^ no-control-regex
    Pattern.compile("a  b");
//  ^^^^^^^^^^^^^^^^^^^^^^^ no-regex-spaces
    Pattern.matches("[a" + "\u0301]", s);
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-misleading-character-class
    Pattern.compile("a b");
    Other.compile("a  b");
    Pattern.compile(s);
  }
}
