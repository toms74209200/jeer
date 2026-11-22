@interface A {
  int X = 1;

  String[] v() default {};

  int[] w() default {1, 2};

  Class<?> c() default Object.class;

  @interface B {}
}
