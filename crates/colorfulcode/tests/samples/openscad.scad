// Parametric box with a lid
include <BOSL2/std.scad>
use <utils/rounded.scad>

$fn = 64;
WALL = 2.0;
box_size = [60, 40, 25];
label = "Parts \"A\"";

/* Hollow box body */
module box(size = box_size, wall = WALL, open_top = true) {
    difference() {
        cube(size);
        translate([wall, wall, wall])
            cube([size.x - 2 * wall, size.y - 2 * wall, size.z]);
    }
}

function inner(size, wall) = [for (s = size) s - 2 * wall];

module lid(size = box_size) {
    color("SteelBlue", 0.8)
    translate([0, 0, size.z + 5])
        linear_extrude(height = WALL)
            offset(r = 1) square([size.x, size.y]);
}

box();
lid();
for (i = [0 : 3]) {
    rotate([0, 0, i * 90]) translate([10, 0, 0]) sphere(d = 4);
}
echo(str("inner size: ", inner(box_size, WALL), " ", label));
if (len(box_size) != 3) assert(false, "bad size");
%cylinder(h = 10, r = 1.5e0, center = true);
