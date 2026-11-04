#include <iostream>
#include <vector>
#include <memory>

namespace geometry {

template <typename T>
concept Numeric = std::is_arithmetic_v<T>;

template <Numeric T>
class Vec2 final {
public:
    constexpr Vec2(T x, T y) noexcept : x_(x), y_(y) {}
    [[nodiscard]] T dot(const Vec2& other) const { return x_ * other.x_ + y_ * other.y_; }
    auto operator+(const Vec2& o) const -> Vec2 { return {x_ + o.x_, y_ + o.y_}; }

private:
    T x_, y_;
};

}  // namespace geometry

int main() {
    using geometry::Vec2;
    auto points = std::make_unique<std::vector<Vec2<double>>>();
    points->emplace_back(1.5, 2.0);
    const char* raw = R"json({"key": "value"})json";
    for (const auto& p : *points) {
        std::cout << p.dot(p) << ' ' << raw << std::endl;
    }
    static_assert(sizeof(int) >= 4, "int too small");
    return nullptr == nullptr ? 0 : 1;
}
