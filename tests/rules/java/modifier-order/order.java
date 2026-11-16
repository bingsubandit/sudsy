class A {
  static public final int A = 1;
//       ^^^^^^ modifier-order
  public static final int B = 1;
  @Deprecated final private int c = 1;
//                  ^^^^^^^ modifier-order
  private final @Deprecated int d = 1;
  synchronized public void f() {}
//             ^^^^^^ modifier-order
}
