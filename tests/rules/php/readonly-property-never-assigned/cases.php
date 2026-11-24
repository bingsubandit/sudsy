<?php
class A { public readonly int $a; public function __construct() { $this->a = 1; } }
class B { private readonly int $b; public function set() { $this->b = 1; } }
//        ^^^^^^^ readonly-property-never-assigned
class C { public function __construct(public readonly int $c) {} }
class D { use T; public readonly int $d; }
class E { public function __construct() { $this->e = 1; } public readonly int $e; public readonly int $f; }
//                                                                                ^^^^^^ readonly-property-never-assigned
class G { private readonly int $g; private readonly int $h; function __construct() { [$this->g, $this->h] = [1, 2]; } }
