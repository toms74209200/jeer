module com.example.app {
  requires transitive java.sql;

  exports com.example.api to
      com.example.client;

  opens com.example.impl;

  uses com.example.Spi;

  provides com.example.Spi with
      com.example.Impl;
}
