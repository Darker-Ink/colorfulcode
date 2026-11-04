package com.example.inventory;

import java.util.*;
import java.util.stream.Collectors;

/**
 * Tracks items in a warehouse.
 */
@SuppressWarnings("unchecked")
public final class Inventory<T extends Item> implements Iterable<T> {
    private static final int MAX_ITEMS = 10_000;
    private final Map<String, List<T>> byCategory = new HashMap<>();
    private long total = 0L;

    public record Item(String name, double price, char grade) {}

    public enum Status { IN_STOCK, SOLD_OUT }

    @Override
    public Iterator<T> iterator() {
        return byCategory.values().stream().flatMap(List::stream).iterator();
    }

    public void add(String category, T item) throws IllegalStateException {
        if (total >= MAX_ITEMS) {
            throw new IllegalStateException("inventory full: " + total + "\n");
        }
        byCategory.computeIfAbsent(category, k -> new ArrayList<>()).add(item);
        this.total++;
    }

    public String report() {
        var names = byCategory.keySet().stream().sorted().collect(Collectors.joining(", "));
        String text = """
            Report:
              categories: %s
            """.formatted(names);
        return switch (total > 0 ? Status.IN_STOCK : Status.SOLD_OUT) {
            case IN_STOCK -> text;
            case SOLD_OUT -> null;
        };
    }

    public static void main(String[] args) {
        System.out.println(0x1F + 3.5e2 + 'c');
    }
}
