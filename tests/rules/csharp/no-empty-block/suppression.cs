class A { void F() {
 // sudsy-disable-next-line no-empty-block
 try { } catch { }
 try { } catch { }
//   ^^^ no-empty-block
//             ^^^ no-empty-block
} }
