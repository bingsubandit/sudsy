if (a === a) { g(); }
//  ^^^^^^^ no-self-compare
if (true) { g(); }
//  ^^^^ no-constant-condition
if (a = b) { g(); }
//  ^^^^^ no-assignment-in-condition
if (1 === 2) { g(); }
//  ^^^^^^^ no-constant-comparison
try { g(); } catch (e) { }
//                     ^^^ no-empty-block
