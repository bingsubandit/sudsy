class A { void F() {
    int k = 5;
//      ^ unused-variable
    int a = 1, b = 2;
    try { F(); } catch (System.Exception e) { }
//                                       ^ unused-variable
//                                          ^^^ no-empty-block
    System.Console.WriteLine(b + a);
} }
