package com.example.shop;

import static java.util.Objects.requireNonNull;

import java.util.List;

/** A shop. */
public class Shop {
  private final List<String> items;

  public Shop(List<String> items) {
    this.items = requireNonNull(items);
  }

  public int count(boolean onlyNonEmpty) {
    if (!onlyNonEmpty) {
      return items.size();
    }
    int n = 0;
    for (String item : items) {
      if (!item.isEmpty()) {
        n++;
      }
    }
    return n;
  }
}
