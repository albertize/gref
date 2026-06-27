use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_name(prefix: &str) -> String {
    format!(
        "{}_{}",
        prefix,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

#[test]
fn vim_runtime_parses_args_and_result_protocol() {
    if Command::new("vim").arg("--version").output().is_err() {
        eprintln!("skipping Vim runtime test: vim not found");
        return;
    }

    let dir = std::env::temp_dir().join(unique_name("gref_vim_runtime"));
    fs::create_dir_all(&dir).unwrap();
    let script_path = dir.join("test.vim");
    let result_path = dir.join("result.txt");
    let autoload_path = std::env::current_dir()
        .unwrap()
        .join("contrib/vim/autoload/gref.vim");

    let script = format!(
        r#"
set nomore
execute 'source ' . fnameescape({autoload:?})

function! AssertEqual(expected, actual, label) abort
  if string(a:expected) !=# string(a:actual)
    call writefile(['FAIL ' . a:label, 'expected: ' . string(a:expected), 'actual: ' . string(a:actual)], {result:?})
    cquit 1
  endif
endfunction

let funcs = execute('function')
let prefix = matchstr(funcs, '<SNR>\d\+_ShellWords')
let prefix = substitute(prefix, 'ShellWords$', '', '')
if empty(prefix)
  call writefile(['FAIL could not locate gref script-local functions'], {result:?})
  cquit 1
endif

call AssertEqual(['foo bar', 'baz qux', 'plain arg'], call(prefix . 'ShellWords', ['"foo bar" ''baz qux'' plain\ arg']), 'shell words quote parsing')

let parsed = call(prefix . 'ParseArgs', ['--regex "foo bar" "baz qux" src'], 0)
call AssertEqual(['--regex', '--root', 'src'], parsed.options, 'parse options and positional root')
call AssertEqual('foo bar', parsed.pattern, 'parse pattern')
call AssertEqual(1, parsed.is_replace, 'parse replace mode')
call AssertEqual('baz qux', parsed.replacement, 'parse replacement')

let parsed_dash = call(prefix . 'ParseArgs', ['--ignore-case -- -starts-with-dash'], 0)
call AssertEqual(['--ignore-case', '--'], parsed_dash.options, 'parse end-of-options marker')
call AssertEqual('-starts-with-dash', parsed_dash.pattern, 'parse dash pattern')

let selected_file = tempname()
call writefile(['selected', '12', '5', 'dir/file with space.rs'], selected_file, 'b')
let selected = call(prefix . 'ReadResult', [selected_file])
call AssertEqual('selected', selected.status, 'selected status')
call AssertEqual(12, selected.line, 'selected line')
call AssertEqual(5, selected.column, 'selected column')
call AssertEqual('dir/file with space.rs', selected.path, 'selected path')
call AssertEqual(0, filereadable(selected_file), 'selected result is deleted')

let error_file = tempname()
call writefile(['error', 'line one', 'line two'], error_file, 'b')
let err = call(prefix . 'ReadResult', [error_file])
call AssertEqual('error', err.status, 'error status')
call AssertEqual("line one\nline two", err.message, 'error message')

let jump_dir = tempname()
call mkdir(jump_dir, 'p')
let jump_file = jump_dir . '/utf8.txt'
call writefile(['é foo'], jump_file)
let jump_result = tempname()
call writefile(['selected', '1', '4', jump_file], jump_result, 'b')
let g:gref_open_command = 'edit'
call call(prefix . 'OpenResult', [jump_result, win_getid(), 0, 10, 0])
call AssertEqual(fnamemodify(jump_file, ':p'), expand('%:p'), 'open selected file')
call AssertEqual(1, line('.'), 'open selected line')
call AssertEqual(4, col('.'), 'open selected byte column')

call writefile(['ok'], {result:?})
qa!
"#,
        autoload = autoload_path.to_string_lossy(),
        result = result_path.to_string_lossy()
    );

    fs::write(&script_path, script).unwrap();
    let output = Command::new("vim")
        .args(["-Nu", "NONE", "-n", "-es", "-S"])
        .arg(&script_path)
        .output()
        .unwrap();

    let result = fs::read_to_string(&result_path).unwrap_or_default();
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(
        result.trim(),
        "ok",
        "Vim runtime test failed\nexit status: {}\nresult:\n{}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        result,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
