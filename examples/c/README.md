# C example

Highlights a file through the C API in `crates/colorfulcode-ffi/include/colorfulcode.h`
and prints it with 24 bit terminal colors taken from `cc_theme_style`.

```sh
make            # builds the static library with cargo, then links ./highlight
./highlight main.c dracula
```

`make run` highlights `main.c` itself. Pass a theme name as the second argument
(see `colorfulcode --list-themes`).
