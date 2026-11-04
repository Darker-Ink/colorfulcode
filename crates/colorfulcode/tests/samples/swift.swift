import SwiftUI

/// A to-do item.
struct Todo: Identifiable, Codable {
    let id: UUID
    var title: String
    var done = false
}

enum Filter: String, CaseIterable {
    case all, open, finished
}

@MainActor
final class TodoStore: ObservableObject {
    @Published private(set) var items: [Todo] = []
    private let maxItems = 1_000

    func add(_ title: String) throws {
        guard items.count < maxItems else {
            throw StoreError.full(limit: maxItems)
        }
        items.append(Todo(id: UUID(), title: title))
    }

    func visible(for filter: Filter) -> [Todo] {
        switch filter {
        case .all: return items
        case .open: return items.filter { !$0.done }
        case .finished: return items.filter(\.done)
        }
    }
}

enum StoreError: Error {
    case full(limit: Int)
}

struct TodoRow: View {
    let todo: Todo

    var body: some View {
        Text("\(todo.title) is \(todo.done ? "done" : "open")\n")
            .font(.body)
    }
}

#if DEBUG
let pattern = #"\d+ "quoted""#
let ratio = 0.5e-3 + Double(0xFF)
#endif
