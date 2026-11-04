//! Targeted checks for shells, build tools and their configuration files.

mod common;

use colorfulcode::Kind::*;
use colorfulcode::Language;
use common::{assert_not, assert_tokens};

#[test]
fn bash_is_unchanged() {
    assert_tokens(
        "bash",
        "if [ -f \"$HOME/.rc\" ]; then echo ${X:-1}; fi",
        &[("if", Keyword), ("-f", Attribute), ("$HOME", Variable), ("echo", Builtin), ("${X:-1}", Variable)],
    );
}

#[test]
fn powershell_variables_cmdlets_and_parameters() {
    let code = "$files = Get-ChildItem -Path $env:TEMP -Recurse | Where-Object { $_.Length -gt 1KB }";
    assert_tokens(
        "powershell",
        code,
        &[
            ("$files", Variable),
            ("Get-ChildItem", Function),
            ("-Path", Attribute),
            ("$env:TEMP", Variable),
            ("-Recurse", Attribute),
            ("Where-Object", Function),
            ("$_", Variable),
            ("Length", Property),
            ("-gt", Operator),
            ("1KB", Number),
        ],
    );
    assert_tokens(
        "pwsh",
        "if ($a -and -not $b) { $true }",
        &[("-and", Operator), ("-not", Operator), ("$true", Constant)],
    );
    assert_tokens(
        "ps1",
        "-ceq -inotlike -replace",
        &[("-ceq", Operator), ("-inotlike", Operator), ("-replace", Operator)],
    );
}

#[test]
fn powershell_strings_and_comments() {
    assert_tokens(
        "powershell",
        "\"Hi $name, `$literal $($user.Name)\"",
        &[
            ("\"Hi ", String),
            ("$name", Variable),
            ("`$", Escape),
            ("$(", Punctuation),
            ("$user", Variable),
            ("Name", Property),
            (")", Punctuation),
        ],
    );
    // Backslashes are not escapes in PowerShell.
    assert_tokens("powershell", "$p = \"C:\\temp\\\"; $q", &[("\"C:\\temp\\\"", String), ("$q", Variable)]);
    assert_tokens("powershell", "'it''s'", &[("'it''s'", String)]);
    assert_tokens(
        "powershell",
        "@\"\nline \"quoted\" $x\n\"@\n$y",
        &[("$x", Variable), ("\n\"@", String), ("$y", Variable)],
    );
    assert_tokens(
        "powershell",
        "<# block\ncomment #>\n# line",
        &[("<# block\ncomment #>", Comment), ("# line", Comment)],
    );
}

#[test]
fn powershell_types_and_functions() {
    assert_tokens(
        "powershell",
        "[CmdletBinding()]\nparam([string[]]$Names)\nfunction Get-Thing { [Math]::Round(1.5) }",
        &[
            ("CmdletBinding", Meta),
            ("param", Keyword),
            ("string[]", Type),
            ("$Names", Variable),
            ("function", Keyword),
            ("Get-Thing", Function),
            ("Math", Type),
            ("Round", Function),
        ],
    );
    assert_not("powershell", "$items[0]", "0", Type);
    assert_tokens("powershell", "$items[0]", &[("0", Number)]);
}

#[test]
fn fish_functions_and_set() {
    let code = "function greet --argument name\n    set -l msg \"hi $name\"\n    echo $msg (date)\nend";
    assert_tokens(
        "fish",
        code,
        &[
            ("function", Keyword),
            ("greet", Function),
            ("--argument", Attribute),
            ("set", Builtin),
            ("-l", Attribute),
            ("msg", Variable),
            ("$name", Variable),
            ("echo", Builtin),
            ("date", Function),
            ("end", Keyword),
        ],
    );
    assert_not("fish", "set_color red", "set_color", Variable);
}

#[test]
fn batch_comments_labels_and_variables() {
    let code = "@echo off\nREM a comment\n:: another\n:loop\nset /a n=%n%+1\ngoto :loop";
    assert_tokens(
        "batch",
        code,
        &[
            ("@", Operator),
            ("echo", Builtin),
            ("REM a comment", Comment),
            (":: another", Comment),
            (":loop", Function),
            ("/a", Attribute),
            ("n", Variable),
            ("%n%", Variable),
            ("goto", Keyword),
            (":loop", Function),
        ],
    );
    assert_tokens(
        "bat",
        "for %%f in (*.txt) do echo %%~nf !count! \"%~dp0out\"",
        &[("%%f", Variable), ("%%~nf", Variable), ("!count!", Variable), ("%~dp0", Variable), ("out\"", String)],
    );
    // `rem` inside a word or argument is not a comment.
    assert_not("cmd", "echo remove it", "remove it", Comment);
    assert_tokens("cmd", "echo a ^| b", &[("^|", Escape)]);
}

#[test]
fn dockerfile_instructions_and_shell() {
    let code = "FROM node:20 AS build\nENV PORT=8080\nRUN npm ci && \\\n    echo \"$PORT\"\nCMD [\"node\", \"app.js\"]";
    assert_tokens(
        "dockerfile",
        code,
        &[
            ("FROM", Keyword),
            ("AS", Keyword),
            ("ENV", Keyword),
            ("PORT", Variable),
            ("RUN", Keyword),
            ("npm", Function),
            ("&&", Operator),
            ("echo", Builtin),
            ("$PORT", Variable),
            ("CMD", Keyword),
            ("\"node\"", String),
        ],
    );
    assert_tokens(
        "docker",
        "RUN --mount=type=cache,target=/x apt-get update\n# comment\nonbuild run make",
        &[
            ("--mount", Attribute),
            ("apt-get", Function),
            ("# comment", Comment),
            ("onbuild", Keyword),
            ("run", Keyword),
            ("make", Function),
        ],
    );
    assert_tokens("dockerfile", "RUN <<EOF\nset -e\nEOF\nUSER app", &[("<<", Operator), ("USER", Keyword)]);
    assert_eq!(Language::from_path("app/Dockerfile").unwrap().id(), "dockerfile");
    assert_eq!(Language::from_path("Containerfile").unwrap().id(), "dockerfile");
}

#[test]
fn makefile_rules_variables_and_recipes() {
    let code = "CC ?= gcc\nOBJS := $(SRCS:.c=.o)\n.PHONY: all\nall: app\n\t@echo \"built $@ $$HOME\"\n\t$(CC) -o $@ $^";
    assert_tokens(
        "makefile",
        code,
        &[
            ("CC", Variable),
            ("?=", Operator),
            ("OBJS", Variable),
            ("$(SRCS:.c=.o)", Variable),
            (".PHONY", Keyword),
            ("all", Function),
            ("all", Function),
            ("@", Operator),
            ("echo", Builtin),
            ("$@", Variable),
            ("$$HOME", Variable),
            ("$(CC)", Variable),
            ("-o", Attribute),
            ("$^", Variable),
        ],
    );
    assert_tokens(
        "make",
        "FILES = $(wildcard *.c)\nNOW := $(shell date +%s)\nifeq ($(OS),Linux)\nendif",
        &[
            ("$(", Punctuation),
            ("wildcard", Function),
            ("shell", Function),
            ("date", Function),
            ("ifeq", Keyword),
            ("$(OS)", Variable),
            ("endif", Keyword),
        ],
    );
    // A tab indented line outside a rule is not a recipe.
    assert_not("makefile", "X = 1\n\tY = 2", "Y", Function);
    assert_eq!(Language::from_path("GNUmakefile").unwrap().id(), "makefile");
}

#[test]
fn cmake_commands_and_variables() {
    let code = "function(greet who)\n  message(STATUS \"hi ${who} $ENV{USER}\")\nendfunction()\nset(FLAGS -Wall)";
    assert_tokens(
        "cmake",
        code,
        &[
            ("function", Keyword),
            ("greet", Function),
            ("message", Function),
            ("STATUS", Keyword),
            ("${who}", Variable),
            ("$ENV{USER}", Variable),
            ("endfunction", Keyword),
            ("FLAGS", Variable),
            ("-Wall", Attribute),
        ],
    );
    assert_tokens(
        "cmake",
        "#[[ multi\nline ]] [=[ raw ]=]",
        &[("#[[ multi\nline ]]", Comment), ("[=[ raw ]=]", String)],
    );
    assert_tokens("cmake", "$<$<CONFIG:Debug>:x>", &[("$<$<", Meta), ("CONFIG", Text)]);
    assert_eq!(Language::from_path("src/CMakeLists.txt").unwrap().id(), "cmake");
}

#[test]
fn nginx_directives_and_variables() {
    let code =
        "server {\n  listen 80;\n  location / { return 301 https://$host$request_uri; }\n  add_header X \"v=$uri\";\n}";
    assert_tokens(
        "nginx",
        code,
        &[
            ("server", Keyword),
            ("listen", Keyword),
            ("80", Number),
            ("location", Keyword),
            ("return", Keyword),
            // Adjacent tokens of the same kind merge.
            ("$host$request_uri", Variable),
            ("add_header", Keyword),
            ("$uri", Variable),
        ],
    );
    assert_tokens("nginx", "allow 10.0.0.1; gzip on;", &[("10.0.0.1", Number), ("on", Constant)]);
}

#[test]
fn apache_sections_and_rewrite_flags() {
    let code = "<Directory \"/var/www\">\n  RewriteRule ^(.*)$ /x/$1 [L,R=301]\n</Directory>";
    assert_tokens(
        "apacheconf",
        code,
        &[
            ("Directory", Tag),
            ("RewriteRule", Keyword),
            ("$1", Variable),
            ("L", Attribute),
            ("R", Attribute),
            ("301", Number),
            ("</", Punctuation),
            ("Directory", Tag),
        ],
    );
    assert_tokens("apacheconf", "# c\nRedirect / http://x/#frag", &[("# c", Comment)]);
    assert_not("apacheconf", "Redirect / http://x/#frag", "#frag", Comment);
    assert_eq!(Language::from_path("site/.htaccess").unwrap().id(), "apacheconf");
}

#[test]
fn awk_fields_and_patterns() {
    let code = "BEGIN { FS = \",\" }\n/error/ { n++ }\n$3 > 10 { print $1, NF }";
    assert_tokens(
        "awk",
        code,
        &[
            ("BEGIN", Keyword),
            ("FS", Variable),
            ("/error/", Regex),
            ("$3", Variable),
            ("print", Keyword),
            ("$1", Variable),
            ("NF", Variable),
        ],
    );
    assert_tokens("awk", "x = a / b / c", &[("/", Operator)]);
}

#[test]
fn nix_strings_paths_and_attributes() {
    let code =
        "{ pkgs ? import <nixpkgs> {} }:\nlet\n  name = \"hi ${pkgs.lib}\";\n  src = ./src;\nin rec { inherit name; }";
    assert_tokens(
        "nix",
        code,
        &[
            ("import", Builtin),
            ("<nixpkgs>", String),
            ("let", Keyword),
            ("name", Property),
            ("${", Punctuation),
            ("lib", Property),
            ("./src", String),
            ("rec", Keyword),
            ("inherit", Keyword),
        ],
    );
    assert_tokens("nix", "x = ''\n  multi ${y}\n'';", &[("''\n  multi ", String), ("y", Text), ("\n''", String)]);
    assert_tokens("nix", "a = b / c;", &[("/", Operator)]);
}

#[test]
fn just_recipes_and_interpolation() {
    let code = "set shell := [\"bash\", \"-c\"]\nver := \"1\"\n\n# build it\nbuild target='x': lint\n    cargo build {{target}}\n    @echo \"v{{ver}}\"";
    assert_tokens(
        "just",
        code,
        &[
            ("set", Keyword),
            ("shell", Property),
            ("ver", Variable),
            (":=", Operator),
            ("# build it", Comment),
            ("build", Function),
            ("target", Variable),
            ("'x'", String),
            ("lint", Function),
            ("cargo", Function),
            ("{{", Punctuation),
            ("@", Operator),
            ("echo", Builtin),
            ("{{", Punctuation),
            ("ver", Text),
        ],
    );
    assert_tokens("just", "py:\n    #!/usr/bin/env python3\n    print(1)", &[("print", Builtin)]);
    assert_eq!(Language::from_path("Justfile").unwrap().id(), "just");
}

#[test]
fn meson_calls_and_keywords() {
    assert_tokens(
        "meson",
        "exe = executable('app', sources, install : true)\nforeach f : files\nendforeach",
        &[
            ("executable", Function),
            ("'app'", String),
            ("install", Property),
            ("true", Constant),
            ("foreach", Keyword),
            ("f", Variable),
            ("endforeach", Keyword),
        ],
    );
    assert_eq!(Language::from_path("sub/meson.build").unwrap().id(), "meson");
}

#[test]
fn ninja_rules_and_builds() {
    let code = "rule cc\n  command = gcc -c $in -o $out\n  pool = heavy\nbuild a.o: cc a.c\nbuilddir = build";
    assert_tokens(
        "ninja",
        code,
        &[
            ("rule", Keyword),
            ("cc", Function),
            ("command", Property),
            ("$in", Variable),
            ("pool", Property),
            ("build", Keyword),
            (":", Operator),
            ("cc", Function),
            ("builddir", Property),
        ],
    );
    assert_not("ninja", "builddir = build", "build", Keyword);
    assert_tokens("ninja", "build a$ b: phony", &[("$ ", Escape)]);
}

#[test]
fn vim_commands_comments_and_scopes() {
    let code = "\" settings\nset number tabstop=4\nlet g:loaded = 1\nfunction! s:Init() abort\n  call s:Run(a:1) \" trailing\nendfunction";
    assert_tokens(
        "vim",
        code,
        &[
            ("\" settings", Comment),
            ("set", Keyword),
            ("number", Property),
            ("tabstop", Property),
            ("4", Number),
            ("let", Keyword),
            ("g:loaded", Variable),
            ("function", Keyword),
            ("s:Init", Function),
            ("call", Keyword),
            ("s:Run", Function),
            ("a:1", Variable),
            ("\" trailing", Comment),
        ],
    );
    assert_tokens("vim", "echo \"a string\"", &[("\"a string\"", String)]);
    assert_tokens("vim", "nnoremap <leader>w :w<CR>", &[("<leader>", Constant), ("<CR>", Constant)]);
    assert_not("vim", "call foo(bar)", "bar", Function);
}

#[test]
fn gitignore_patterns() {
    assert_tokens(
        "gitignore",
        "# deps\nnode_modules/\n*.log\n!keep.log\n\\#file\nbuild-[0-9]/",
        &[("# deps", Comment), ("/", Punctuation), ("*", Operator), ("!", Operator), ("\\#", Escape), ("[0-9]", Regex)],
    );
    assert_not("gitignore", "a#b", "#b", Comment);
    assert_eq!(Language::from_path(".dockerignore").unwrap().id(), "gitignore");
}

#[test]
fn gitconfig_sections_and_values() {
    let code = "[remote \"origin\"]\n\turl = git@x:y.git\n[alias]\n\tlg = !git log --oneline\n\tbare = true ; note";
    assert_tokens(
        "gitconfig",
        code,
        &[
            ("remote", Type),
            ("\"origin\"", String),
            ("url", Property),
            ("=", Operator),
            ("lg", Property),
            ("!", Operator),
            ("git", Function),
            ("--oneline", Attribute),
            ("true", Constant),
            ("; note", Comment),
        ],
    );
    assert_eq!(Language::from_path(".gitmodules").unwrap().id(), "gitconfig");
}

#[test]
fn gitattributes_attributes() {
    assert_tokens(
        "gitattributes",
        "*.sh text eol=lf\n*.png -diff !merge\n[attr]bin -text",
        &[
            ("*", Operator),
            ("text", Attribute),
            ("eol", Attribute),
            ("lf", String),
            ("-", Operator),
            ("diff", Attribute),
            ("!", Operator),
            ("[attr]", Keyword),
            ("bin", Function),
        ],
    );
}

#[test]
fn sshconfig_hosts_and_options() {
    assert_tokens(
        "sshconfig",
        "Host *.corp !gw\n  Port 2222\n  User=ops\n  ControlPath ~/.ssh/%r@%h\n  ProxyCommand ssh -W %h:%p gw",
        &[
            ("Host", Keyword),
            ("*", Operator),
            (".corp", Type),
            ("!", Operator),
            ("gw", Type),
            ("Port", Property),
            ("2222", Number),
            ("User", Property),
            ("=", Operator),
            ("%r", Variable),
            ("%h", Variable),
            ("ProxyCommand", Property),
            ("ssh", Function),
            ("-W", Attribute),
        ],
    );
}

#[test]
fn caddyfile_sites_directives_and_placeholders() {
    let code = "example.com {\n\t@api path /api/*\n\treverse_proxy @api localhost:8080\n\trespond \"hi {host}\" 200\n}";
    assert_tokens(
        "caddyfile",
        code,
        &[
            ("example.com", Type),
            ("{", Punctuation),
            ("@api", Meta),
            ("reverse_proxy", Keyword),
            ("@api", Meta),
            ("respond", Keyword),
            ("{host}", Variable),
            ("200", Number),
            ("}", Punctuation),
        ],
    );
    assert_tokens("caddy", "(snip) {\n\theader -Server\n}", &[("snip", Function), ("header", Keyword)]);
}

#[test]
fn systemd_units() {
    assert_tokens(
        "systemd",
        "[Service]\nExecStart=-/usr/bin/app --port ${PORT} %i\nRestart=on-failure\nRestartSec=5s\nNoNewPrivileges=yes",
        &[
            ("Service", Type),
            ("ExecStart", Property),
            ("=-", Operator),
            ("/usr/bin/app", Function),
            ("--port", Attribute),
            ("${PORT}", Variable),
            ("%i", Variable),
            ("5s", Number),
            ("yes", Constant),
        ],
    );
    assert_eq!(Language::from_path("/etc/systemd/system/app.service").unwrap().id(), "systemd");
}

#[test]
fn crontab_schedules() {
    assert_tokens(
        "crontab",
        "MAILTO=root\n*/5 * * * 1-5 /usr/bin/job --quiet\n@reboot echo up",
        &[
            ("MAILTO", Variable),
            ("*/", Operator),
            ("5", Number),
            ("1", Number),
            ("-", Operator),
            ("/usr/bin/job", Function),
            ("--quiet", Attribute),
            ("@reboot", Keyword),
            ("echo", Builtin),
        ],
    );
}

#[test]
fn procfile_processes() {
    assert_tokens(
        "procfile",
        "web: bundle exec puma -p $PORT",
        &[("web", Property), ("bundle", Function), ("-p", Attribute), ("$PORT", Variable)],
    );
}

#[test]
fn xonsh_subprocess_and_python() {
    let code = "def f(x):\n    return $(ls -la @(x))\n$PATH.append('/bin')\n![echo $HOME]";
    assert_tokens(
        "xonsh",
        code,
        &[
            ("def", Keyword),
            ("f", Function),
            ("$(", Punctuation),
            ("ls", Function),
            ("-la", Attribute),
            ("@(", Punctuation),
            ("x", Text),
            ("$PATH", Variable),
            ("append", Function),
            ("![", Punctuation),
            ("echo", Builtin),
            ("$HOME", Variable),
        ],
    );
    assert_tokens("xonsh", "a != b", &[("!=", Operator)]);
}

#[test]
fn nushell_definitions_and_pipelines() {
    let code =
        "def greet [name: string] {\n    let msg = $\"hi ($name)\"\n    ls | where size > 1kb | each {|f| $f.name }\n}";
    assert_tokens(
        "nushell",
        code,
        &[
            ("def", Keyword),
            ("greet", Function),
            ("name", Property),
            ("string", Type),
            ("let", Keyword),
            ("msg", Variable),
            ("$\"hi ", String),
            ("(", Punctuation),
            ("$name", Variable),
            ("ls", Builtin),
            ("where", Keyword),
            ("1kb", Number),
            ("each", Builtin),
            ("f", Variable),
            ("$f", Variable),
        ],
    );
    assert_tokens("nu", "r#'raw \\n'# --flag", &[("r#'raw \\n'#", String), ("--flag", Attribute)]);
}

#[test]
fn aliases_and_file_names() {
    let cases = [
        ("ps1", "powershell"),
        ("pwsh", "powershell"),
        ("bat", "batch"),
        ("cmd", "batch"),
        ("docker", "dockerfile"),
        ("make", "makefile"),
        ("mk", "makefile"),
        ("nu", "nushell"),
        ("viml", "vim"),
    ];
    for (alias, id) in cases {
        assert_eq!(Language::find(alias).unwrap().id(), id, "{alias}");
    }
    let paths = [
        ("Makefile", "makefile"),
        ("nginx.conf", "nginx"),
        ("Caddyfile", "caddyfile"),
        (".vimrc", "vim"),
        (".gitconfig", "gitconfig"),
        (".gitattributes", "gitattributes"),
        ("Procfile", "procfile"),
        ("x.timer", "systemd"),
        ("build.ninja", "ninja"),
        ("default.nix", "nix"),
        ("script.awk", "awk"),
        ("config.fish", "fish"),
        ("run.xsh", "xonsh"),
    ];
    for (path, id) in paths {
        assert_eq!(Language::from_path(path).unwrap().id(), id, "{path}");
    }
}
