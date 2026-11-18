<?php

$a = preg_match('/\x1f/', $s);
//   ^^^^^^^^^^^^^^^^^^^^^^^^ no-control-regex
$b = preg_replace('/a  b/i', '', $s);
//   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-regex-spaces
$c = preg_match('/a b/', $s);
$d = preg_foo('/a  b/', $s);
$e = preg_match('/[a' . '\x{301}]/u', $s);
//   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-misleading-character-class
