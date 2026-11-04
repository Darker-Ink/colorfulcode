using System;
using System.Collections.Generic;
using System.Threading.Tasks;

namespace Shop.Orders;

#nullable enable

/// <summary>An order placed by a customer.</summary>
[Serializable]
public sealed class Order
{
    public const int MaxLines = 50;
    private readonly List<OrderLine> _lines = new();

    public Guid Id { get; init; } = Guid.NewGuid();
    public string? Customer { get; set; }
    public decimal Total => _lines.Sum(l => l.Price * l.Quantity);

    public record OrderLine(string Sku, decimal Price, int Quantity);

    [Obsolete("Use AddAsync instead")]
    public void Add(OrderLine line)
    {
        if (_lines.Count >= MaxLines) throw new InvalidOperationException($"Too many lines: {_lines.Count}");
        _lines.Add(line);
    }

    public async Task<bool> AddAsync(OrderLine line, [CallerMemberName] string caller = "")
    {
        await Task.Delay(10);
        Add(line);
        return true;
    }

    public override string ToString()
    {
        var path = @"C:\orders\""archive""";
        var json = $$"""{"id": "{{Id}}"}""";
        var c = '\n';
        return $"Order {Id:N} for {Customer ?? "nobody"}: {Total,10:C2}{c}{path}{json}";
    }
}

[Flags]
public enum Access : byte
{
    None = 0,
    /// Can read (and list).
    Read = 1 << 0,
    Write = 1 << 1,
    All = Read | Write,
}

public static class Program
{
    public static void Main(string[] args)
    {
        var order = new Order { Customer = "Ada" };
        order.Add(new Order.OrderLine("A-1", 9.99m, 2));
        Console.WriteLine(order);
    }
}
