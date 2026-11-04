// Every test sample from the Rust crate doubles as demo content.
const files = import.meta.glob("../../../crates/colorfulcode/tests/samples/*", {
  query: "?raw",
  import: "default",
  eager: true,
});

export const samples = Object.fromEntries(
  Object.entries(files).map(([path, code]) => [path.split("/").pop().split(".")[0], code]),
);
