import java.util.List;

class A {
  int f(int a, List<String> xs) {
    int unused = 1;
        ^^^^^^ unused-local-variable
    int written = 0;
        ^^^^^^^ unused-local-variable
    written = 2;
    int used = a;
    int counter = 0;
    counter++;
    for (String s : xs) {
      used += s.length();
    }
    for (int i = 0; i < 3; i++) {
    }
    Runnable r = () -> {
             ^ unused-local-variable
      int inner = 1;
          ^^^^^ unused-local-variable
    };
    return used;
  }
}
