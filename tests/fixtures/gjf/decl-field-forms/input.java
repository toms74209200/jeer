class A {
  int a[] = {1}, b = 2, c;
  int[] d = {1, 2};
  int[][] e = {{1, 2}, {3, 4}};
  String veryLongFieldNameNumberOne = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
  java.util.Map<String, java.util.List<int[]>> m;
  Outer<String>.Inner<Integer> n;
  java.util.Map.Entry<String, Integer> o;
  java.util.List<?> p;
  java.util.List<? extends Number> q;
  java.util.List<? super Integer> r;
  transient volatile int s;
}
