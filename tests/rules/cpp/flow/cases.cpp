int a() {
  try {
    int* p = new int(1);
    return *p;
  } catch (const E&) {
    return -1;
  }
}
int b() {
  return 1;
  g();
//^^^^ unreachable-code
}
