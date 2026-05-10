" Vim integration for gref. Copy this file to ~/.vim/plugin/gref.vim or use
" Vim's native package layout under ~/.vim/pack/gref/start/gref/plugin/.

if exists('g:loaded_gref')
  finish
endif
let g:loaded_gref = 1

command! -nargs=+ Gref call gref#run(<q-args>, 0)
command! -nargs=+ GrefBuffer call gref#run(<q-args>, 1)
