class A {
  int f(int n, int m) {
    n = 1;
    ^ parameter-assignment
    n++;
    ^ parameter-assignment
    int k = m;
    k = 2;
    return k + n;
  }
}
