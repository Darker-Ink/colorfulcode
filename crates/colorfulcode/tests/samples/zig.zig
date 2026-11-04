const std = @import("std");
const Allocator = std.mem.Allocator;

/// A growable stack of items.
pub fn Stack(comptime T: type) type {
    return struct {
        const Self = @This();

        items: std.ArrayList(T),

        pub fn init(allocator: Allocator) Self {
            return .{ .items = std.ArrayList(T).init(allocator) };
        }

        pub fn push(self: *Self, value: T) !void {
            try self.items.append(value);
        }

        pub fn pop(self: *Self) ?T {
            return self.items.popOrNull();
        }
    };
}

const Color = enum(u8) { red = 1, green, blue };

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    var stack = Stack(i32).init(gpa.allocator());
    try stack.push(0x2A);
    const banner =
        \\Hello from a
        \\multiline string
    ;
    const c: u8 = 'z';
    if (stack.pop()) |top| {
        std.debug.print("top = {d}, color = {s}\n", .{ top, @tagName(Color.green) });
    } else unreachable;
    _ = banner;
    _ = c;
}

test "push and pop" {
    try std.testing.expect(true and 1.5e3 > 0);
}
