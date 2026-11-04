#!/usr/bin/env fish
# Jump to a project directory with fuzzy matching.

set -g PROJECTS_DIR $HOME/code
set -l count 0

function proj --description 'cd into a project'
    set -l query $argv[1]
    if test -z "$query"
        echo "usage: proj <name>" >&2
        return 1
    end

    for dir in $PROJECTS_DIR/*/
        if string match -q "*$query*" (basename $dir)
            cd $dir
            set count (math $count + 1)
            echo "Now in $(pwd) ($count)"
            return 0
        end
    end

    switch $query
        case 'dot*'
            cd ~/.dotfiles
        case '*'
            echo 'no match' >&2; and return 2
    end
end

abbr -a gco 'git checkout'
if status is-interactive; and not set -q TMUX
    set_color green; echo "fish $version ready"; set_color normal
end
