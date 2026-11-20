/** Good summary. Then more */
class A {
  /** No period */
//^^^ doc-summary
  void a() {}

  /** lowercase start. */
//^^^ doc-summary
  void b() {}

  /**
   * Two lines
   * of summary.
   */
  void c() {}

  /** {@inheritDoc} */
  void d() {}

  /** Version 1.5 is fine. */
  void e() {}

  /**
//^^^ doc-summary
   * @param x only a tag
   */
  void f() {}

  /** A {@code Foo} is a thing. */
//^^^ doc-summary
  void g() {}

  /** Ends with a question? */
//^^^ doc-summary
  void h() {}
}
