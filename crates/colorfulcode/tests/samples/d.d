module app.matrix;

import std.stdio;
import std.algorithm : map, sum;
import std.conv : to;

/+ A nested /+ comment +/ still inside. +/
enum MAX_SIZE = 64;

/// A small dense matrix.
struct Matrix(T) if (__traits(isArithmetic, T))
{
    private T[] data;
    immutable size_t rows, cols;

    this(size_t rows, size_t cols) @safe pure nothrow
    {
        this.rows = rows;
        this.cols = cols;
        data = new T[rows * cols];
    }

    ref T opIndex(size_t r, size_t c) return
    {
        return data[r * cols + c];
    }

    T trace() const @nogc
    {
        T total = 0;
        foreach (i; 0 .. rows)
            total += data[i * cols + i];
        return total;
    }
}

class Shape
{
    abstract double area();
}

unittest
{
    auto m = Matrix!double(2, 2);
    m[0, 0] = 1.5;
    assert(m.trace() == 1.5);
}

void main(string[] args)
{
    auto squares = [1, 2, 3].map!(x => x * x).sum;
    string raw = r"C:\temp\" ~ `no\escapes`;
    char c = '\n';
    writefln("squares = %d, args = %s%c", squares, args.length.to!string, c);
    writeln(raw, 0xFF_FF, 3.14e-2);
}
