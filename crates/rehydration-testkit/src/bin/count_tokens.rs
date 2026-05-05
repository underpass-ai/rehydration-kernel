use std::{env, fs, process};

fn main() {
    let paths: Vec<String> = env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: count_tokens <file> [<file> ...]");
        process::exit(2);
    }

    let bpe = tiktoken_rs::cl100k_base().expect("cl100k_base vocabulary should load");

    println!("file,tokens,bytes");
    for path in paths {
        let text = fs::read_to_string(&path).unwrap_or_else(|error| {
            eprintln!("failed to read {path}: {error}");
            process::exit(1);
        });
        let tokens = bpe.encode_ordinary(&text).len();
        println!("{path},{tokens},{}", text.len());
    }
}
