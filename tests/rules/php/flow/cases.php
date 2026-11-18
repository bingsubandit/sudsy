<?php
function a($x) {
    return 1;
    b();
//  ^^^^ unreachable-code
    c();
}
function d($x) {
    foreach ($x as $y) { continue; e(); }
//                                 ^^^^ unreachable-code
    while (true) { if ($x) { break; } }
    f();
    for (;;) { }
//           ^^^ no-empty-block
    g();
//  ^^^^ unreachable-code
}
function h($x) {
    try { return 1; } catch (E $e) { }
//                    ^^^^^^^^^^^^^^^^ no-empty-catch
//                                 ^^^ unreachable-code
    i();
    switch ($x) {
        case 1:
            j();
        case 2:
//      ^ no-fallthrough
            k();
            break;
        case 3:
        case 4:
            l();
            // falls through
        default:
            m();
    }
}
