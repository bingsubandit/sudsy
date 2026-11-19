resource "a" "b" {
  name = var.name
  tags = ["${var.a}", "x-${var.b}"]
}
