@Deprecated()
class A {
  @Override() public String toString() { return ""; }
  @SuppressWarnings ( ) int x;
  @A()int y;
  @B(/* c */) int z;
  @C(@D()) int w;
}
