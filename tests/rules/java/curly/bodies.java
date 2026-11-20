class A {
  void f(boolean x, int[] xs) {
    if (x) return;
//  ^ curly
    if (x) {
//  ^ curly
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
//  ^ curly
    for (int v : xs) g();
//  ^ curly
    while (x) g();
//  ^ curly
    do g(); while (x);
//  ^ curly
    for (int v : xs) {
      g();
    }
  }
}
