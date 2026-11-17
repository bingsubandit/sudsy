class A {
  void F(int a, int b) {
    if (a == a) { G(); }
//      ^^^^^^ no-self-compare
    if (true) { G(); }
//      ^^^^ no-constant-condition
    while (true) { G(); }
    if (a = b) { G(); }
//      ^^^^^ no-assignment-in-condition
    if (1 == 2) { G(); }
//      ^^^^^^ no-constant-comparison
    try { G(); } catch (Exception) { }
//                                 ^^^ no-empty-block
  }
}
