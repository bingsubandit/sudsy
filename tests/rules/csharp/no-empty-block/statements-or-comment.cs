class A { void F() {
 try { Run(); } catch (Exception) { }
//                                ^^^ no-empty-block
 try { Run(); } catch { /* ignored on purpose */ }
 try { Run(); } catch { Log(); }
 try { Run(); } catch { try { Run(); } catch { Log(); } }
} }
