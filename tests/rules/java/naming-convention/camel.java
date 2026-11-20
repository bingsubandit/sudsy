class A {
  void ok() {}
  void getUrl() {}
  void a() {}
//     ^ naming-convention
  void getURL() {}
//     ^^^^^^ naming-convention
  void foo_bar() {}
//     ^^^^^^^ naming-convention
  void foo1_2() {}

  @Test
  void testFoo_bar() {}

  @Test
  void test_x() {}
//     ^^^^^^ naming-convention

  @org.junit.jupiter.params.ParameterizedTest
  void test_1() {}
//     ^^^^^^ naming-convention

  @TestFoo
  void test_z() {}
//     ^^^^^^ naming-convention
}
