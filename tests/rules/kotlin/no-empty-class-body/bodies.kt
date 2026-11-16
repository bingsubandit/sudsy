class A {}
//      ^^ no-empty-class-body
class B { }
//      ^^^ no-empty-class-body
class C {
    val x = 1
}
interface D {}
//          ^^ no-empty-class-body
val o = object : Any() {}
