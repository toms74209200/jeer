sealed class A<T> extends B<T> implements C, D permits E, F {}

class B<T extends Comparable<T> & java.io.Serializable> implements C {}

interface I<@A T> extends J {}

class C<T> {}

non-sealed class D extends A<String> {}

final class E extends A<Integer> {}
