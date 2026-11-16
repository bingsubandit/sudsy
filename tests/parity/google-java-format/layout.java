package a.b;

import static java.util.Objects.requireNonNull;

import java.util.List;

/** A class. */
@SuppressWarnings("unused")
public final class Layout<T extends Comparable<T>> implements Iterable<T> {
  private static final int[] NUMBERS = {1, 2, 3};
  private final List<T> items;

  Layout(List<T> items) { this.items = requireNonNull(items); }

  public <R> R fold(R init, java.util.function.BiFunction<R, ? super T, R> f, boolean parallel, int limit) throws IllegalStateException {
    R acc = init;
    for (T item : items) {
      if (item == null) continue;
      acc = f.apply(acc, item);
    }
    String message = "a very long string literal " + acc + " that has to be broken over several lines " + limit;
    switch (limit) {
      case 1:
        return acc;
      default:
        break;
    }
    return items.stream().filter(x -> x != null).map(Object::toString).collect(java.util.stream.Collectors.joining(", ")).isEmpty();
  }

  enum Color { RED, GREEN, BLUE }
}
