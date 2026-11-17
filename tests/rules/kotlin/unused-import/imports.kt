import a.b.Used
import a.b.Unused
^^^^^^^^^^^^^^^^^ unused-import
import a.b.Orig as Alias
^^^^^^^^^^^^^^^^^^^^^^^^ unused-import
import a.b.Other as Short
import a.b.Doc
import c.d.*
import c.delegate.getValue
import c.ext.helper

/** See [Doc]. */
class A(val used: Used) {
  val v by delegate
  fun f(s: Short) = s.helper()
}
