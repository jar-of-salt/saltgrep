use saltgrep::compile::compile;
use saltgrep::matcher::{Match, Matcher};
use std::env::args_os;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

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

    let searcher = compile(pattern)?;
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

    let mut stdout = StandardStream::stdout(ColorChoice::Auto);
    // ... write to stdout

    contents
        .lines()
        .map(|line| {
            let mut curr_at = 0;
            searcher.try_find_iter_at(line, curr_at, |found| {
                write!(&mut stdout, "{}", &line[curr_at..found.start])?;
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
                write!(&mut stdout, "{}", found.substr(line))?;
                stdout.reset()?;
                curr_at = found.end;
                Ok::<bool, io::Error>(true)
            })?;
            if curr_at != line.len() {
                write!(&mut stdout, "{}", &line[curr_at..line.len()])?;
            }
            writeln!(&mut stdout, "")
        })
        .count();

    Ok(())
}
