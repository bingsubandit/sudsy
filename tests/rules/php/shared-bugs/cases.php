<?php
if ($a == $a) { }
//  ^^^^^^^^ no-self-compare
//            ^^^ no-empty-block
if (true) { g(); }
while ($a = f()) { g(); }
//     ^^^^^^^^ no-assignment-in-condition
$d = ['k' => 1, 'k' => 2];
//              ^^^ no-duplicate-keys
switch ($a) { case 1: break; case 1: break; }
//                           ^^^^^^^^^^^^^^ no-duplicate-case
try { g(); } catch (Exception $e) { }
//           ^^^^^^^^^^^^^^^^^^^^^^^^ no-empty-catch
