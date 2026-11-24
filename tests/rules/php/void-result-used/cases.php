<?php
class A {
    function f(): void {}
    /** @return void */
    protected function d($a) {}
    /** @return int */
    function n() { return 1; }
    function g($x) {
        $y = $this->f();
//           ^^^^^ void-result-used
        $this->f();
        if ($x) {
            return self::d(1);
//                 ^^^^ void-result-used
        }
        return $x ? static::f() : null;
//                  ^^^^^^ void-result-used
    }
    /** @return void|int */
    function w() {}
    function h() { return $this->n() + $this->other() + $this->w(); }
}
