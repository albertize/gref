" Popup-terminal integration for the gref binary.

let s:active_popup = 0

function! s:SetupHighlights() abort
  highlight default GrefPopup ctermbg=NONE guibg=NONE
  highlight default GrefPopupBorder ctermfg=240 guifg=#585858 ctermbg=NONE guibg=NONE
endfunction

function! s:ShellWords(args) abort
  let words = split(a:args)
  if empty(words)
    echoerr 'Gref requires a pattern'
    return []
  endif
  if len(words) > 2
    echoerr 'Gref expects: :Gref {pattern} [{replacement}]'
    return []
  endif
  return words
endfunction

function! s:PopupSize() abort
  let width_percent = get(g:, 'gref_popup_width_percent', 85)
  let height_percent = get(g:, 'gref_popup_height_percent', 80)
  let width = (&columns * width_percent) / 100
  let height = (&lines * height_percent) / 100
  let width = min([max([50, width]), max([20, &columns - 8])])
  let height = min([max([12, height]), max([5, &lines - 6])])
  return [width, height]
endfunction

function! s:PopupOptions(size) abort
  return {
        \ 'minwidth': a:size[0],
        \ 'maxwidth': a:size[0],
        \ 'minheight': a:size[1],
        \ 'maxheight': a:size[1],
        \ 'zindex': 200,
        \ 'pos': 'center',
        \ 'border': get(g:, 'gref_popup_border', []),
        \ 'borderchars': get(g:, 'gref_popup_borderchars', ['─', '│', '─', '│', '╭', '╮', '╯', '╰']),
        \ 'borderhighlight': ['GrefPopupBorder'],
        \ 'highlight': 'GrefPopup',
        \ 'padding': get(g:, 'gref_popup_padding', [0, 0, 0, 0]),
        \ 'title': get(g:, 'gref_popup_title', ''),
        \ 'drag': 1,
        \ 'resize': 1,
        \ }
endfunction

function! s:WithTrailingSlash(path) abort
  if a:path =~# '/$'
    return a:path
  endif
  return a:path . '/'
endfunction

function! s:ModifiedBuffersUnder(root) abort
  let root = s:WithTrailingSlash(fnamemodify(a:root, ':p'))
  let modified = []
  for nr in range(1, bufnr('$'))
    if !bufloaded(nr) || !getbufvar(nr, '&modified')
      continue
    endif
    let name = bufname(nr)
    if empty(name)
      continue
    endif
    let path = fnamemodify(name, ':p')
    if stridx(path, root) == 0
      call add(modified, path)
    endif
  endfor
  return modified
endfunction

function! s:CheckReplaceSafety(buffer_only, cwd) abort
  if !a:buffer_only
    let modified = s:ModifiedBuffersUnder(a:cwd)
    if !empty(modified)
      echoerr 'Gref replace blocked: save or abandon modified buffers under ' . a:cwd
      return 0
    endif
    return 1
  endif

  if empty(expand('%:p'))
    echoerr 'GrefBuffer requires a file-backed buffer'
    return 0
  endif
  if &modified
    echoerr 'GrefBuffer replace blocked: save or abandon the current buffer first'
    return 0
  endif
  return 1
endfunction

function! s:ClosePopup(winid) abort
  if a:winid > 0 && !empty(popup_getpos(a:winid))
    call popup_close(a:winid)
  endif
  if s:active_popup == a:winid
    let s:active_popup = 0
  endif
endfunction

function! s:OpenResult(result_file, return_winid, popup_winid, attempt, timer) abort
  call s:ClosePopup(a:popup_winid)
  if a:return_winid <= 0 || !win_gotoid(a:return_winid) || win_gettype() ==# 'popup'
    if a:attempt < 10
      call timer_start(25, function('s:OpenResult', [a:result_file, a:return_winid, a:popup_winid, a:attempt + 1]))
    else
      echoerr 'Gref could not return focus to the original Vim window'
    endif
    return
  endif

  if !filereadable(a:result_file)
    return
  endif

  let lines = readfile(a:result_file, 'b')
  call delete(a:result_file)
  if len(lines) < 2
    return
  endif

  let lnum = str2nr(lines[0])
  let path = join(lines[1:], "\n")
  if lnum <= 0 || empty(path)
    return
  endif

  execute 'edit +' . lnum . ' ' . fnameescape(path)
endfunction

function! s:AfterReplace(result_file, return_winid, popup_winid, attempt, timer) abort
  if filereadable(a:result_file)
    call delete(a:result_file)
  endif
  call s:ClosePopup(a:popup_winid)
  if a:return_winid <= 0 || !win_gotoid(a:return_winid) || win_gettype() ==# 'popup'
    if a:attempt < 10
      call timer_start(25, function('s:AfterReplace', [a:result_file, a:return_winid, a:popup_winid, a:attempt + 1]))
    else
      echoerr 'Gref could not return focus to the original Vim window'
    endif
    return
  endif
  silent! checktime
endfunction

function! s:OnExit(result_file, is_replace, return_winid, ctx, job, status) abort
  let a:ctx.done = 1
  let popup_winid = get(a:ctx, 'popup', 0)
  call s:ClosePopup(popup_winid)
  if a:is_replace
    call timer_start(25, function('s:AfterReplace', [a:result_file, a:return_winid, popup_winid, 0]))
  else
    call timer_start(25, function('s:OpenResult', [a:result_file, a:return_winid, popup_winid, 0]))
  endif
endfunction

function! gref#run(args, buffer_only) abort
  if !has('terminal') || !has('popupwin')
    echoerr 'Gref requires Vim with +terminal and +popupwin'
    return
  endif
  call s:SetupHighlights()
  if s:active_popup
    echoerr 'Gref is already running'
    return
  endif

  let words = s:ShellWords(a:args)
  if empty(words)
    return
  endif

  let is_replace = len(words) == 2
  let cwd = getcwd()
  let return_winid = win_getid()
  if is_replace && !s:CheckReplaceSafety(a:buffer_only, cwd)
    return
  endif

  let binary = get(g:, 'gref_binary', 'gref')
  if !executable(binary)
    echoerr 'gref binary not found: ' . binary
    return
  endif

  let result_file = tempname()
  let argv = [binary, '--vim-result', result_file]
  if a:buffer_only
    let current_file = expand('%:p')
    if empty(current_file)
      echoerr 'GrefBuffer requires a file-backed buffer'
      return
    endif
    call add(argv, '--root')
    call add(argv, current_file)
  endif

  call add(argv, words[0])
  if is_replace
    call add(argv, words[1])
  endif

  let size = s:PopupSize()
  let ctx = {'popup': 0, 'done': 0}
  let buf = term_start(argv, {
        \ 'hidden': 1,
        \ 'term_finish': 'close',
        \ 'term_name': 'gref',
        \ 'cwd': cwd,
        \ 'term_cols': size[0],
        \ 'term_rows': size[1],
        \ 'exit_cb': function('s:OnExit', [result_file, is_replace, return_winid, ctx]),
        \ })
  if buf <= 0
    echoerr 'failed to start gref'
    return
  endif

  let winid = popup_create(buf, s:PopupOptions(size))
  if winid <= 0
    execute 'bwipeout!' buf
    echoerr 'failed to open gref popup'
    return
  endif

  let s:active_popup = winid
  let ctx.popup = winid
  if get(ctx, 'done', 0)
    call s:ClosePopup(winid)
    return
  endif
  silent! call win_execute(winid, 'startinsert')
endfunction
