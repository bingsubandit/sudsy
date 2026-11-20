/** Documented. */
public class Api {
  private int x;
  public Api() {}
//^^^^^^ missing-doc-comment

  /** Documented. */
  public void ok() {}

  public void f() {}
//^^^^^^ missing-doc-comment

  @Deprecated
//^ missing-doc-comment
  protected void g() {}

  @Override
  public String toString() { return ""; }

  @Test
  public void t() {}

  public int getX() { return x; }
  public void setX(int x) { this.x = x; }
  public int getY(int y) { return y; }
//^^^^^^ missing-doc-comment

  void packagePrivate() {}
  private void hidden() {}
  /** Nested. */
  public interface Inner {
    void member();
//  ^^^^ missing-doc-comment
  }

  public static class NoDoc {
//^^^^^^ missing-doc-comment
    public void inNested() {}
//  ^^^^^^ missing-doc-comment
  }

  public enum Kind {
//^^^^^^ missing-doc-comment
    A;

    public void inEnum() {}
//  ^^^^^^ missing-doc-comment
  }

  protected static class Prot {
//^^^^^^^^^ missing-doc-comment
    public void inProt() {}
  }

  void run() {
    Object o = new Object() {
      public void anon() {}
    };
    class Local {
      public void local() {}
    }
  }
}

class PackagePrivate {
  public void notPublicType() {}
}
