import java.util.List;
import java.util.Map;
^^^^^^^^^^^^^^^^^^^^^ unused-import
import java.util.Set;
import java.io.*;
import static java.lang.Math.max;
import static java.lang.Math.min;
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ unused-import

/** Uses {@link Set}. */
class A {
  List<String> xs;

  int f() {
    return max(1, 2);
  }
}
