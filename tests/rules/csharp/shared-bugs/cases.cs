class A
{
    void F(int a, bool b)
    {
        if (a == a) { }
//          ^^^^^^ no-self-compare
//                  ^^^ no-empty-block
        if (true) { b = !b; }
//          ^^^^ no-constant-condition
        while (b = a > 1) { b = false; }
//             ^^^^^^^^^ no-assignment-in-condition
        switch (a) { case 1: break; case 1: break; }
//                                  ^^^^^^^^^^^^^^ no-duplicate-case
        try { b = true; } catch { }
//                              ^^^ no-empty-block
        var c = 1 == 2;
//              ^^^^^^ no-constant-comparison
    }
}
