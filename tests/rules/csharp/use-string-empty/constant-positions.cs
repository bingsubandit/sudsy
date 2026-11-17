class A {
 const string C = "";
 string F(string s = "") {
  if (s is "") { }
//             ^^^ no-empty-block
  switch (s) { case "": break; }
  return s ?? @"";
//            ^^^ use-string-empty
 }
 [System.Obsolete("")] void G() { }
}
