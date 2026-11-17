class A { void F() {
 // sudsy-disable-next-line no-empty-block
 try { Run(); } catch { }
 try { Run(); } catch { }
//                    ^^^ no-empty-block
} }
