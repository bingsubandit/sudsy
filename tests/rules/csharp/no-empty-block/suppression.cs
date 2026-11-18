class A { void F() {
 // sudsy-disable-next-line no-empty-block
 if (F()) { }
 // sudsy-disable-next-line no-empty-catch
 try { } catch { }
//   ^^^ no-empty-block
 if (F()) { }
//        ^^^ no-empty-block
} }
