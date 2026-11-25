<?php
namespace App;
class A {
    public function f(Missing $x): Gone
    {
        return new Gone();
    }
}
