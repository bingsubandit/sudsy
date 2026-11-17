<?php

function f(int $a, $b = 1) {
    $x = $a + $b;
    [$p, $q] = g();
    foreach ($arr as $k => $v) {
             ^^^^ undefined-variable
        echo $k . $v . $p . $q . $x;
    }
    try {
        h($m);
    } catch (Exception $e) {
        echo $e;
    }
    $r = function () use ($x) {
        return $x + $undefined;
                    ^^^^^^^^^^ undefined-variable
    };
    $s = fn($z) => $z + $x;
    global $gg;
    static $st = 0;
    $x++;
    preg_match('/a/', $a, $m2);
    $list[] = 1;
    echo $gg . $st . $m2 . $list . Foo::$bar . $this->z . $_GET['a'];
    echo isset($maybe) ? "$x {$y}" : '';
                              ^^ undefined-variable
    return $s;
}

function g() {
    return $top;
           ^^^^ undefined-variable
}

function h() {
    extract($data);
    return $name;
}

$top = 1;
echo $top . $nowhere;
