class A
{
    void F(string s)
    {
        var a = new Regex("\x1f");
//              ^^^^^^^^^^^^^^^^^ no-control-regex
        var b = Regex.IsMatch(s, @"a  b");
//              ^^^^^^^^^^^^^^^^^^^^^^^^^ no-regex-spaces
        var c = new Regex("[a" + "\u0301]");
//              ^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-misleading-character-class
        var d = Regex.IsMatch("a  b", s);
        var e = new Regex("a b");
    }
}
