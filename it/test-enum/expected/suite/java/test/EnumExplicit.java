package test;

import java.util.Objects;

public enum EnumExplicit {
  A("foo"),
  B("bar");

  private final String value;

  private EnumExplicit(
    final String value
  ) {
    Objects.requireNonNull(value, "value");
    this.value = value;
  }

  public static EnumExplicit fromValue(final String value) {
    for (final EnumExplicit v_value : values()) {
      if (v_value.value.equals(value)) {
        return v_value;
      }
    }

    throw new IllegalArgumentException("value");
  }

  public String toValue() {
    return this.value;
  }
}
