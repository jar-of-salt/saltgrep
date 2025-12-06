use std::env;
use std::fs;

fn main() {
    let mut args: Vec<String> = env::args().collect();
    args = dbg!(args);

    // let query = &args[1];
    let file_path = &args[2];

    let contents = fs::read_to_string(file_path).expect("File failed to read");

    let matches = contents.lines().enumerate().collect::<Vec<(usize, &str)>>();

    for str_match in matches {
        println!("Line {} -- {}", str_match.0, str_match.1);
    }
}
