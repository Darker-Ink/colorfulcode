/* A Reason React component with variants and pattern matching. */
open Belt;

type status('a) =
  | Loading
  | Loaded('a)
  | Failed(string);

type user = {
  name: string,
  age: int,
};

// Labelled arguments and a default value.
let greet = (~name, ~greeting="Hello", ()) => greeting ++ ", " ++ name ++ "!\n";

let rec factorial = n => n <= 1 ? 1 : n * factorial(n - 1);

let describe = (status: status(user)) =>
  switch (status) {
  | Loading => "loading..."
  | Loaded({name, age}) when age >= 18 => name ++ " (adult)"
  | Loaded({name}) => name
  | Failed(message) => {j|failed: $message|j}
  };

let initial = 'R';
let colors = [`Red, `Green, `Blue];
let total = List.reduce([1, 2, 0x10], 0, (a, b) => a + b);

module Counter = {
  [@react.component]
  let make = (~label: string) => {
    let (count, setCount) = React.useState(() => 0);
    <div className="counter">
      <button onClick={_ => setCount(c => c + 1)}> {React.string(label)} </button>
      <span> {React.int(count)} </span>
    </div>;
  };
};

let user = {name: "Ada", age: 36};
Js.log(describe(Loaded(user)));
Js.log(greet(~name=user.name, ()));
