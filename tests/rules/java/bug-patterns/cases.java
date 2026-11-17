class A {
  void f(int a, int b) {
    if (a == a) { g(); }
//      ^^^^^^ no-self-compare
    if (true) { g(); }
//      ^^^^ no-constant-condition
    while (true) { g(); }
    if (a = b) { g(); }
//      ^^^^^ no-assignment-in-condition
    if (1 == 2) { g(); }
//      ^^^^^^ no-constant-comparison
    try { g(); } catch (Exception e) { }
//                                   ^^^ no-empty-block
  }
}
