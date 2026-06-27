" Popup-terminal integration for the gref binary.

let s:active_popup = 0

function! s:SetupHighlights() abort
  highlight default GrefPopup ctermbg=NONE guibg=NONE
  highlight default GrefPopupBorder ctermfg=240 guifg=#585858 ctermbg=NONE guibg=NONE
endfunction

function! s:ShellWords(args) abort
  let words = []
  let current = ''
  let quote = ''
  let escaped = 0
  let has_token = 0
  let i = 0

  while i < strlen(a:args)
    let ch = strpart(a:args, i, 1)
    if escaped
      let current .= ch
      let escaped = 0
      let has_token = 1
    elseif ch ==# '\'
      let escaped = 1
      let has_token = 1
    elseif !empty(quote)
      if ch ==# quote
        let quote = ''
      else
        let current .= ch
      endif
      let has_token = 1
    elseif ch ==# '"' || ch ==# "'"
      let quote = ch
      let has_token = 1
    elseif ch =~# '\s'
      if has_token
        call add(words, current)
        let current = ''
        let has_token = 0
      endif
    else
      let current .= ch
      let has_token = 1
    endif
    let i += 1
  endwhile

  if escaped
    let current .= '\'
  endif
  if !empty(quote)
    echoerr 'Gref unmatched quote'
    return []
  endif
  if has_token
    call add(words, current)
  endif
  return words
endfunction

function! s:ArgsList(args) abort
  if type(a:args) == v:t_list
    return copy(a:args)
  endif
  if type(a:args) == v:t_string
    return s:ShellWords(a:args)
  endif
  echoerr 'Gref internal error: expected argument list'
  return []
endfunction

function! s:DefaultArgs() abort
  let args = get(g:, 'gref_default_args', [])
  if type(args) == v:t_list
    return copy(args)
  endif
  if type(args) == v:t_string
    return empty(args) ? [] : s:ShellWords(args)
  endif
  echoerr 'g:gref_default_args must be a List'
  return []
endfunction

function! s:ParseArgs(args, buffer_only) abort
  let raw = s:DefaultArgs()
  call extend(raw, s:ArgsList(a:args))
  let options = []
  let positional = []
  let root_seen = 0
  let i = 0

  while i < len(raw)
    let arg = raw[i]
    if arg ==# '--'
      call add(options, arg)
      call extend(positional, raw[i + 1:])
      break
    elseif index(['-i', '--ignore-case', '-r', '--regex', '--hidden', '--no-ignore'], arg) >= 0
      call add(options, arg)
    elseif arg ==# '-e' || arg ==# '--exclude'
      let i += 1
      if i >= len(raw)
        echoerr 'Gref option requires a value: ' . arg
        return {}
      endif
      call add(options, arg)
      call add(options, raw[i])
    elseif arg ==# '--root'
      if a:buffer_only
        echoerr 'GrefBuffer does not accept --root'
        return {}
      endif
      let i += 1
      if i >= len(raw)
        echoerr 'Gref option requires a value: --root'
        return {}
      endif
      let root_seen = 1
      call add(options, arg)
      call add(options, raw[i])
    elseif arg =~# '^-'
      echoerr 'Gref unknown option: ' . arg
      return {}
    else
      call add(positional, arg)
    endif
    let i += 1
  endwhile

  if empty(positional)
    echoerr 'Gref requires a pattern'
    return {}
  endif
  if len(positional) > 3 || (a:buffer_only && len(positional) > 2)
    echoerr a:buffer_only
          \ ? 'GrefBuffer expects: :GrefBuffer [options] {pattern} [{replacement}]'
          \ : 'Gref expects: :Gref [options] {pattern} [{replacement}] [directory]'
    return {}
  endif

  if !a:buffer_only && len(positional) == 3
    if root_seen
      echoerr 'Gref root specified twice'
      return {}
    endif
    let dash_index = index(options, '--')
    if dash_index >= 0
      call insert(options, positional[2], dash_index)
      call insert(options, '--root', dash_index)
    else
      call add(options, '--root')
      call add(options, positional[2])
    endif
    call remove(positional, 2)
  endif

  return {
        \ 'options': options,
        \ 'pattern': positional[0],
        \ 'is_replace': len(positional) == 2,
        \ 'replacement': len(positional) == 2 ? positional[1] : '',
        \ }
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

function! s:CanonicalPath(path) abort
  let path = fnamemodify(a:path, ':p')
  let resolved = resolve(path)
  return empty(resolved) ? path : resolved
endfunction

function! s:ModifiedBuffersUnder(root) abort
  let root = s:WithTrailingSlash(s:CanonicalPath(a:root))
  let modified = []
  for nr in range(1, bufnr('$'))
    if !bufloaded(nr) || !getbufvar(nr, '&modified')
      continue
    endif
    let name = bufname(nr)
    if empty(name)
      continue
    endif
    let path = s:CanonicalPath(name)
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

function! s:ReadResult(result_file) abort
  if !filereadable(a:result_file)
    return {'status': 'missing'}
  endif

  let lines = readfile(a:result_file, 'b')
  call delete(a:result_file)
  if empty(lines)
    return {'status': 'missing'}
  endif

  let status = lines[0]
  if status ==# 'selected'
    if len(lines) < 4
      return {'status': 'cancelled'}
    endif
    return {
          \ 'status': 'selected',
          \ 'line': str2nr(lines[1]),
          \ 'column': str2nr(lines[2]),
          \ 'path': join(lines[3:], "\n"),
          \ }
  endif
  if status ==# 'none'
    return {'status': 'none'}
  endif
  if status ==# 'error'
    return {'status': 'error', 'message': join(lines[1:], "\n")}
  endif
  if status ==# 'replaced'
    return {'status': 'replaced'}
  endif
  if status ==# 'cancelled'
    return {'status': 'cancelled'}
  endif

  if status =~# '^\d\+$' && len(lines) >= 2
    return {
          \ 'status': 'selected',
          \ 'line': str2nr(status),
          \ 'column': 1,
          \ 'path': join(lines[1:], "\n"),
          \ }
  endif

  return {'status': 'error', 'message': 'Gref returned an unknown result status: ' . status}
endfunction

function! s:OpenCommand() abort
  let command = get(g:, 'gref_open_command', 'edit')
  if index(['edit', 'split', 'vsplit', 'tabedit', 'drop'], command) < 0
    echoerr 'Invalid g:gref_open_command: ' . command
    return ''
  endif
  return command
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

  let result = s:ReadResult(a:result_file)
  if result.status ==# 'missing' || result.status ==# 'cancelled'
    return
  endif
  if result.status ==# 'none'
    echo 'Gref: no results'
    return
  endif
  if result.status ==# 'error'
    echoerr get(result, 'message', 'Gref failed')
    return
  endif
  if result.status !=# 'selected'
    return
  endif

  let lnum = get(result, 'line', 0)
  let column = max([1, get(result, 'column', 1)])
  let path = get(result, 'path', '')
  if lnum <= 0 || empty(path)
    return
  endif

  let command = s:OpenCommand()
  if empty(command)
    return
  endif
  execute command . ' +' . lnum . ' ' . fnameescape(path)
  call cursor(lnum, column)
endfunction

function! s:AfterReplace(result_file, return_winid, popup_winid, attempt, timer) abort
  call s:ClosePopup(a:popup_winid)
  if a:return_winid <= 0 || !win_gotoid(a:return_winid) || win_gettype() ==# 'popup'
    if a:attempt < 10
      call timer_start(25, function('s:AfterReplace', [a:result_file, a:return_winid, a:popup_winid, a:attempt + 1]))
    else
      echoerr 'Gref could not return focus to the original Vim window'
    endif
    return
  endif
  let result = s:ReadResult(a:result_file)
  if result.status ==# 'error'
    echoerr get(result, 'message', 'Gref failed')
  elseif result.status ==# 'none'
    echo 'Gref: no results'
  elseif result.status ==# 'replaced'
    silent! checktime
  endif
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

  let parsed = s:ParseArgs(a:args, a:buffer_only)
  if empty(parsed)
    return
  endif

  let is_replace = parsed.is_replace
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
  call extend(argv, parsed.options)
  if a:buffer_only
    let current_file = expand('%:p')
    if empty(current_file)
      echoerr 'GrefBuffer requires a file-backed buffer'
      return
    endif
    call add(argv, '--root')
    call add(argv, current_file)
  endif

  call add(argv, parsed.pattern)
  if is_replace
    call add(argv, parsed.replacement)
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
