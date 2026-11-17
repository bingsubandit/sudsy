class A { void F(bool x, int y) {
 ;
 if (x); else { }
 while (x) /* c */ { }
//         ^^^^^^^ no-comment-before-block
//                 ^^^ no-empty-block
 switch (y) { case 1: ; break; default: ; }
 L: ;
 int z = 1;;
} }
