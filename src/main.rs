use clap::{value_parser, Arg, ArgAction, ArgMatches, Command};
use term_size;
use rayon::prelude::*;
use walkdir::WalkDir;
use std::thread;
use std::sync::mpsc::{channel,Receiver};

fn main() {
    main_exec();
}

/// mainの実行
fn main_exec() {
    let matches: ArgMatches = Command::new("l")
        .author("mochizuki")
        .version("0.1.0")
        .about("l command")
        .arg(
            Arg::new("path")
                .help("path")
                .short('p')
                .long("path")
                .value_parser(value_parser!(String)),
        )
        .arg(
            Arg::new("full")
                .help("full path")
                .short('f')
                .long("full")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("recursive")
                .help("recursive list")
                .short('r')
                .long("recursive")
                .action(ArgAction::SetTrue)
                .conflicts_with("full"),
        )
        .arg(
            Arg::new("wide")
                .help("wide list")
                .short('w')
                .long("wide")
                .action(ArgAction::SetTrue)
                .conflicts_with("full")
                .conflicts_with("recursive"),
        )
        .get_matches();

    // カレントディレクトリの取得
    let serch_dir = matches
        .get_one::<String>("path")
        .map_or(get_current_dir(), |path| path.clone());

    if std::path::Path::new(&serch_dir).exists() == false {
        println!("{}", "Error: パスが存在しません");
        return;
    }

    //　引数による条件分岐
    let is_full = matches.get_flag("full");
    let is_recursive = matches.get_flag("recursive");
    let is_wide = matches.get_flag("wide");

    match (is_recursive, is_full, is_wide) {
        (true, false, false) => {
            // ls -r
            let rx_itr = get_recursive_files(serch_dir);
            for it in rx_itr {
               println!("{}",it);
            }
        }
        (false, true, false) => {
            // ls -f
            let files = get_dir_files(serch_dir);
            for file in files {
                println!("{}", file);
            }
        }
        (false, false, true) => {
            // ls -w
            let files = get_dir_files(serch_dir);
            if let Ok(width) = get_term_width() {
                wide_line_print(&files, width);
            } else {
                println!("ターミナルの幅が取得できませんでした");
            }
        }
        (false, false, false) => {
            let mut tmp_files = Vec::<String>::new();
            let files = get_dir_files(serch_dir);
            for file in files {
                tmp_files.push(ceil_path(file));
            }
            tmp_files.sort();
            for file in tmp_files {
                println!("{}", file);
            }
        } // ls
        (_, _, _) => println!("error:-r -f -w は独立して使うこと "), // error
    }
}

///　再帰的にファイルを取得
///  -r option用
/// # Arguments
/// * `path` - ファイルパス
///
/// # Returns
/// * `Vec<String>` - ファイル一覧
fn get_recursive_files(path: String) ->Receiver<String> {
    let (tx,rx) = channel();
    thread::spawn(move || {
      WalkDir::new(path).into_iter()
         .par_bridge()
         .filter_map(|m|m.ok())
         .filter_map(|m|m.path().canonicalize().ok())
         .filter(|m|m.is_file())
         .map(|m|m.to_string_lossy().into_owned().replace(r"\\?\",""))
         .for_each(|p| {let _ = tx.send(p);});
   });

   rx
}

/// ファイル名のみを取得
/// option無しの場合の表示用
///
/// # Arguments
/// * `path` - ファイルパス
///
/// # Returns
/// * `String` - ファイル名
fn ceil_path(path: String) -> String {
    let path = std::path::Path::new(&path);
    if path.is_dir() {
        let dir_name = path
            .file_name()
            .expect("Error: ファイル名の取得に失敗")
            .to_string_lossy()
            .to_string();
        return format!("[{}]", dir_name);
    } else {
        let file_name = path
            .file_name()
            .expect("Error:　ファイル名の取得に失敗")
            .to_string_lossy()
            .to_string();
        return file_name;
    }
}

/// ファイル一覧をフルパスで取得
/// -f option用
/// # Arguments
/// * `dir` - ディレクトリ
///
/// # Returns
/// * `Vec<String>` - ファイル一覧
fn get_dir_files(dir: String) -> Vec<String> {
    let mut files = Vec::new();

    let entries = std::fs::read_dir(dir).expect("Error:  ディレクトリの読み込みに失敗");

    for entry in entries {
        let entry = entry.expect("Error: エントリーの取得に失敗");
        let path = entry.path();
        let path = path.to_str().expect("Error: パスから文字列への変換に失敗");

        files.push(path.to_string());
    }
    return files;
}

/// カレントディレクトリの取得
/// -p 以外の共通処理
/// # Returns
/// * `String` - カレントディレクトリ
fn get_current_dir() -> String {
    let current_dir = std::env::current_dir().expect("Error: カレントディレクトリの取得に失敗");
    let current_dir = current_dir
        .to_str()
        .expect("Error: カレントディレクトリから文字列への変換に失敗");
    return current_dir.to_string();
}

/// ターミナルの幅を取得
/// # Returns
/// * `Result<i32,()>` - ターミナルの幅
fn get_term_width() -> Result<i32, ()> {
    let term_size = term_size::dimensions();

    if let Some((width, _)) = term_size {
        Ok(width as i32)
    } else {
        Err(())
    }
}

/// 文字列ベクターの最大長を取得
/// -w option用
///
/// # Arguments
/// * `strings` - 文字列ベクタ
///
/// # Returns
/// * `i32` - 最大長
fn get_string_max_length(strings: &Vec<String>) -> i32 {
    let string_max_width = strings.iter().map(|s| s.len() as i32).max();
    if let Some(max) = string_max_width {
        max
    } else {
        0
    }
}

/// ワイド表示
/// -w option用
///
/// # Arguments
/// * `file_name_lst` - ファイル名リスト
/// * `width` - 幅
fn wide_line_print(file_name_lst: &Vec<String>, width: i32) {
    /// ワイド表示時のスペース
    const WIDE_ADD_SPACE: i32 = 3;
    let max_filename_length = get_string_max_length(&file_name_lst) + WIDE_ADD_SPACE;
    let separate_nums = width / max_filename_length;
    file_name_lst.iter().enumerate().for_each(|(i, file_name)| {
        // iがseparate_numsの倍数の時に改行
        if i % separate_nums as usize == 0 {
            print!("\n");
        }
        print!(
            "{:width$}",
            ceil_path(file_name.to_string()),
            width = max_filename_length as usize
        );
    });
}
