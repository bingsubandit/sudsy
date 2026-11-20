class A {
  Object a = new int[]{1};
//           ^^^^^^^^^^^^ array-bracket-space
  Object b = new int[] {1};
  Object c = new String[]{"a]{"};
//           ^^^^^^^^^^^^^^^^^^^ array-bracket-space
  Object d = new int[2];
  int[] e = {1};
}
