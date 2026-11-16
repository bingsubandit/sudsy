package a.B;
//      ^^^ naming-convention

class my_class {
//    ^^^^^^^^ naming-convention
  static final int maxSize = 1;
//                 ^^^^^^^ naming-convention
  static final int MAX_SIZE = 1;
  int Count;
//    ^^^^^ naming-convention
  int count;

  void DoIt(int Bad, String... Rest) {
//     ^^^^ naming-convention
//              ^^^ naming-convention
//                             ^^^^ naming-convention
    int Local = 1;
//      ^^^^^ naming-convention
    int local = 2;
    try {
      f();
    } catch (RuntimeException E) {
//    ^^^^^ empty-catch-block
//                            ^ naming-convention
    }
  }

  void ok() {}
}

interface Api {
  int limit = 1;
//    ^^^^^ naming-convention
}
