use saltgrep::compile::compile;
use saltgrep::matcher::{Match, Matcher};
use std::env::args_os;
use std::ffi::OsString;
use std::fs;
use std::io;

const application_name: &str = "saltgrep";

// TODO: help etc; input flags parser
// pattern.to_str().ok_or_else(|| {
//         let valid_up_to = pattern
//             .to_string_lossy()
//             .find('\u{FFFD}')
//             .expect("a Unicode replacement codepoint for invalid UTF-8");
//         InvalidPatternError { original: escape_os(pattern), valid_up_to }
//     })

pub fn main() -> Result<(), io::Error> {
    let args: Vec<OsString> = args_os().collect();

    let pattern_os_string = &args[1];
    let pattern = pattern_os_string.to_str().ok_or_else(|| {
        let valid_up_to = pattern_os_string
            .to_string_lossy()
            .find('\u{FFFD}')
            .expect("a Unicode replacement codepoint for invalid UTF-8");
        io::Error::new(
            io::ErrorKind::Other,
            format!("Bad unicode pattern at {}", valid_up_to),
        )
    })?;
    let file_path = &args[2];

    // let contents = fs::read_to_string(file_path).expect("File failed to read");
    let contents = fs::read_to_string(file_path).expect("saltgrep:");
    // println!("lib: {}", contents);

    let searcher = compile(pattern);
    // println!(
    //     "{:?}",
    //     contents
    //         .lines()
    //         .enumerate()
    //         .map(|(idx, line)| {
    //             println!("line {}", line);
    //             searcher.find(line).map(|match_result| line) // format!("{} :: {}", idx, match_result.substr(line)))
    //         })
    //         .filter(Option::is_some)
    //         .collect::<Vec<Option<String>>>()
    // );

    contents
        .lines()
        .map(|line| {
            searcher.find(line).map(|_| line) // format!("{} :: {}", idx, match_result.substr(line)))
        })
        .filter(Option::is_some)
        .map(|output| println!("{}", output.unwrap()))
        .count();

    Ok(())
}
