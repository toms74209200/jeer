record Point(int x, int y) {
  Point {
    if (x < 0) throw new IllegalArgumentException();
  }

  static Point origin() {
    return new Point(0, 0);
  }
}
