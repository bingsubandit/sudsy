class A {
  int a[], b;
//    ^^^ array-type-style
  int[] c;

  void f(String s[], String[] t, int... u) {
//       ^^^^^^^^^^ array-type-style
    int d[] = new int[1];
//      ^^^^^^^^^^^^^^^^ array-type-style
  }
}
