resource "a" "b" {
  name = "${var.name}"
#        ^^^^^^^^^^^^^ deprecated-interpolation
  tags = ["${var.a}", "x-${var.b}"]
}
