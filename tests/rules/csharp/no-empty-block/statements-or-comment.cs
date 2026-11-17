class A { void F() {
 try { } catch (Exception) { }
//   ^^^ no-empty-block
//                         ^^^ no-empty-block
 try { } catch { /* ignored on purpose */ }
//   ^^^ no-empty-block
 try { } catch { Log(); }
//   ^^^ no-empty-block
 try { } catch { try { } catch { Log(); } }
//   ^^^ no-empty-block
//                   ^^^ no-empty-block
} }
