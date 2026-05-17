use std::{
    env, fs,
    io::{self, Read},
    process,
};

use txt_to_xml_parser::parse;

fn read_source(args: &[String]) -> io::Result<String> {
    match args.get(1).map(String::as_str) {
        None => fs::read_to_string("input.txt"),
        Some("-") => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
        Some(path) => fs::read_to_string(path),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let text = match read_source(&args) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(2);
        }
    };

    let (tree, report) = parse(&text);
    let xml = tree.render();
    print!("{xml}");

    if !report.issues.is_empty() {
        println!("======== Parsing Error Report ========")
    }
    for issue in &report.issues {
        eprintln!("{issue}");
    }

    if !report.issues.is_empty() {
        process::exit(1);
    }
}
