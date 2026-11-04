" Minimal vimrc
set nocompatible
set number relativenumber
set tabstop=4 shiftwidth=4 expandtab
let mapleader = " "
let g:loaded_netrw = 1

" Plugins
call plug#begin('~/.vim/plugged')
Plug 'tpope/vim-surround'
call plug#end()

nnoremap <leader>w :write<CR>
nnoremap <silent> <C-p> :call <SID>OpenFile()<CR>

function! s:OpenFile() abort
  let l:file = input('File: ', '', 'file')
  if empty(l:file)
    echohl WarningMsg | echo "no file" | echohl None
    return
  endif
  execute 'edit ' . fnameescape(l:file)
endfunction

function! StripTrailing(...) range
  let save = winsaveview()
  keeppatterns %s/\s\+$//e
  call winrestview(save)
  echo printf("cleaned %d lines", a:lastline - a:firstline + 1)
endfunction

augroup vimrc
  autocmd!
  autocmd BufWritePre *.py,*.rs call StripTrailing()
  autocmd FileType markdown setlocal spell textwidth=80
augroup END

let s:colors = {'dark': 'gruvbox', 'light': 'morning'}
if has('termguicolors') && &t_Co > 16
  execute 'colorscheme ' . get(s:colors, 'dark')
endif
command! -nargs=1 Grep execute 'silent grep! <args>' | copen
