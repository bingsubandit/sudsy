switch (a) {
  case 1:
    b();
  case 2:
//^ no-fallthrough
    c();
    // falls through
  case 3:
  case 4:
    break;
  default:
    d();
}
