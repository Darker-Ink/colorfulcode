// A small Carbon program that sums a list of shapes.
package Geometry api;

import Math;

interface Shape {
  fn Area[self: Self]() -> f64;
}

class Circle {
  var radius: f64;

  fn Make(r: f64) -> Circle {
    returned var c: Circle = {.radius = r};
    return var;
  }

  impl as Shape {
    fn Area[self: Self]() -> f64 {
      return 3.14159 * self.radius * self.radius;
    }
  }
}

choice Outcome {
  Ok(i32),
  Failed(String)
}

fn Total[T:! Shape](shapes: [T; 3]) -> f64 {
  var sum: f64 = 0.0;
  for (s: T in shapes) {
    sum += s.Area();
  }
  return sum;
}

fn Run() -> i32 {
  let c: Circle = Circle.Make(1.5e0);
  var label: String = "circle\n";
  let doc: String = '''
    Multi line
    block string
    ''';
  if (c.Area() > 0x10 and not false) {
    Print("big {0}", label);
  }
  return 0;
}
