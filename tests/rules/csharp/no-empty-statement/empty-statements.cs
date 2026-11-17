class A { void F(bool x, int y) {
 ;
 ^ no-empty-statement
 if (x); else { }
//     ^ no-empty-statement
//            ^^^ no-empty-block
 while (x) /* c */ { }
//                 ^^^ no-empty-block
 switch (y) { case 1: ; break; default: ; }
//                    ^ no-empty-statement
//                                      ^ no-empty-statement
 L: ;
 int z = 1;;
//         ^ no-empty-statement
} }
