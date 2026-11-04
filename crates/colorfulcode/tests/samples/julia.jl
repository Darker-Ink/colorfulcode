#= Geometry helpers.
   #= nested =# comment =#
module Geometry

export Point, distance

const ORIGIN_NAME = "origin"

"""
    Point(x, y)

A point in the plane.
"""
struct Point{T<:Real}
    x::T
    y::T
end

abstract type Shape end

mutable struct Circle <: Shape
    center::Point{Float64}
    radius::Float64
end

function distance(a::Point, b::Point)::Float64
    return sqrt((a.x - b.x)^2 + (a.y - b.y)^2)
end

area(c::Circle) = pi * c.radius^2

function describe(points::Vector{<:Point})
    for (i, p) in enumerate(points)
        println("Point $i at ($(p.x), $(p.y))\n")
    end
    m = [1 2; 3 4]'
    push!(points, Point(1, 2))
    c = 'x'
    re = r"^\d+$"i
    @assert length(points) > 0 "need points"
    return isempty(points) ? nothing : points[end]
end

end # module
