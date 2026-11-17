class A {
  public boolean equals(Object o) {
//^^^^^^ equals-hash-code
    return o == "s" || o.toString() != "t" || o.hashCode() == 1;
//         ^^^^^^^^ string-literal-equality
//                     ^^^^^^^^^^^^^^^^^^^ string-literal-equality
  }

  boolean ok(boolean done, String s) {
    long big = 10l;
//             ^^^ upper-ell
    long fine = 10L;
    if (done == true || done != false) {
//      ^^^^^^^^^^^^ boolean-literal-comparison
//                      ^^^^^^^^^^^^^ boolean-literal-comparison
      return s.equals("a");
    }
    while (done);
//              ^ empty-statement
    return false;
  }

  void f(int x) {
    try {
      g();
    } catch (Exception e) {
//                        ^ no-empty-block
    }
    try {
      g();
    } catch (Exception e) {
      // ignored
    }
    switch (x) {
//  ^^^^^^ missing-switch-default
      case 1:
        g();
    }
    switch (x) {
      case 1:
        g();
        break;
      default:
        break;
    }
  }
}

class B {
  public boolean equals(Object o) {
    return true;
  }

  public int hashCode() {
    return 1;
  }
}
