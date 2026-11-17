class Cases {
  void f(int a) {
    if (a == a) { g(); }
//      ^^^^^^ no-self-compare
    try { g(); } catch (Exception e) { }
//                                   ^^^ no-empty-block
    if (a > 1) { } else { g(); }
//             ^^^ no-empty-block
  }
}
