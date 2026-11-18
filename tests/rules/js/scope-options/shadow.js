var a = 1, ok = 1, v = 2;
function f(a, ok) { var v = 3; return a + ok + v; }
//         ^ shadowed-variable
