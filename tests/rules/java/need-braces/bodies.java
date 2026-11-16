class A {
  void f(boolean x, int[] xs) {
    if (x) return;
//  ^^ need-braces
    if (x) {
//  ^^ need-braces
      g();
    } else g();
    if (x) {
      g();
    } else if (!x) {
      g();
    } else {
      g();
    }
    for (int i = 0; i < 3; i++) g();
//  ^^^ need-braces
    for (int v : xs) g();
//  ^^^ need-braces
    while (x) g();
//  ^^^^^ need-braces
    do g(); while (x);
//  ^^ need-braces
    for (int v : xs) {
      g();
    }
  }
}
