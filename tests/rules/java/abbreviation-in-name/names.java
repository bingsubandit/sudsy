class XMLParser {
//    ^^^^^^^^^ abbreviation-in-name
  static final int MAX_SIZE = 1;
  int fooBAR;
//    ^^^^^^ abbreviation-in-name
  int goodName;
  int endsWithB;

  void getURL(int httpID) {
//     ^^^^^^ abbreviation-in-name
//                ^^^^^^ abbreviation-in-name
    int myURL = 1;
//      ^^^^^ abbreviation-in-name
    int fine = 2;
  }

  @Override
  public void toJSON() {}
}
