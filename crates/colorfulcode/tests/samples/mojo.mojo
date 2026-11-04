"""Vector math with SIMD."""
from math import sqrt
from memory import UnsafePointer

alias WIDTH = 8


@value
struct Vec3:
    var x: Float64
    var y: Float64
    var z: Float64

    fn __init__(out self, x: Float64, y: Float64, z: Float64):
        self.x = x
        self.y = y
        self.z = z

    fn length(self) -> Float64:
        return sqrt(self.x**2 + self.y**2 + self.z**2)

    fn scale(mut self, factor: Float64):
        self.x *= factor


trait Shape:
    fn area(self) -> Float64:
        ...


fn sum_simd[dtype: DType](data: SIMD[dtype, WIDTH]) raises -> Scalar[dtype]:
    if len(data) == 0:
        raise Error("empty")
    return data.reduce_add()


def main():
    var v = Vec3(1.0, 2.0, 3.5e-1)
    let name: String = 'vec'
    print("Length:", v.length(), "\n")
    for i in range(WIDTH):
        print(i, end=" ")
