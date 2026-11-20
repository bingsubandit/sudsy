/**
 * Good.
 *
 * <p>Fine paragraph.
 *
 * <p> Space after.
// ^^^ doc-paragraph
 *
//^ doc-paragraph
 * Missing tag.
 * <p>Not after an empty line.
// ^^^ doc-paragraph
 *
 * <ul>
 *   <li>item
 * </ul>
 *
 * {@snippet :
 * code();
 *
 * more();
 * }
 *
 * <pre>
 *
 * kept
 * </pre>
 *
 * @param x text
 *
 * after the tags
 */
class A {
  /** <p>First thing. */
//    ^^^ doc-paragraph
  void f() {}
}
