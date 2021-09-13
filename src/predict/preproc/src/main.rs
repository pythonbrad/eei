use std::collections::HashMap;
use fst::{MapBuilder, SetBuilder};
use std::num::ParseIntError;
use std::fs::File;
use std::io;
use std::error;
use std::io::{Write, BufRead};

#[derive(Debug, Clone)]
enum ParseError {
    InvalidJson(String),
    InvalidHex(ParseIntError),
    InvalidCodepoint(u32)
}

//https://stackoverflow.com/questions/69152223/unicode-codepoint-to-rust-string
fn parse_unicode(input: &str) -> Result<char, ParseError> {
    let unicode = u32::from_str_radix(input, 16).map_err(ParseError::InvalidHex)?;
    char::from_u32(unicode).ok_or_else(|| ParseError::InvalidCodepoint(unicode))
}

fn parse_github_emoji_url(url: &String) -> Result<String, ParseError> {
    let bytecode_strings = url.split('/')
        .last().ok_or(ParseError::InvalidJson(url.clone()))?
        .split('.').next().ok_or(ParseError::InvalidJson(url.clone()))?.split('-');

    bytecode_strings.map(|codepoint| parse_unicode(codepoint))
    .collect::<Result<Vec<_>, _>>().map(|char_vec|char_vec.into_iter().collect::<String>())
}

fn github_emoji_shortcodes() -> Vec<(String, String)> {
    let json: HashMap<String, String> = ureq::get("https://api.github.com/emojis").call()
        .unwrap()
        .into_json()
        .unwrap();

    //have to filter out bad URLs like
    // "https://github.githubassets.com/images/icons/emoji/bowtie.png?v8"
    json.iter().filter_map(|(key, url)| {
            parse_github_emoji_url(url).map(|unicode_str| (key.clone(), unicode_str)).ok()
    }).collect::<Vec<(String, String)>>()
}


fn write_symbols_and_shortcodes(data: &Vec<(String, String)>) -> Result<(), Box<dyn error::Error>> {
    let writer = io::BufWriter::new(File::create("shortcodes.fst")?);
    let mut build = MapBuilder::new(writer)?;


    let mut shortcodes_symbols = data.clone();
    shortcodes_symbols.sort_by_key(|(shortcode, symbol)| shortcode.clone());
    let mut counter: u64 = 0;
    let symbol_insertions: Result<Vec<String>, _> = shortcodes_symbols.into_iter()
        .map(|(shortcode, content)| {
            build.insert(shortcode, counter).map(|_| {
                counter += 1;
                content.clone()
            })
    }).collect();
    let symbols = symbol_insertions?;
    // Finish construction of the map and flush its contents to disk.
    build.finish()?;

    let mut symbol_file = File::create("symbols.bin")?;
    symbol_file.write_all(&bincode::serialize(&symbols)?)?;
    Ok(())
}

fn process_dictionary() -> Result<(), Box<dyn error::Error>> {
    //TODO: gonna have to sort/reorder the dictionary

    let writer = io::BufWriter::new(File::create("dictionary.fst")?);
    let mut build = SetBuilder::new(writer)?;


    for line in io::BufReader::new(File::open("hunspell_US.txt")?).lines() {
        match line {
            Ok(line_content) => Ok(build.insert(line_content)?), //convert error type
            Err(e) => Err(e)
        }?
    }
    build.finish()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn error::Error>>{
    println!("Fetching shortcodes from github");
    let shortcodes = github_emoji_shortcodes();

    println!("Writing symbols and shortcodes to files");
    write_symbols_and_shortcodes(&shortcodes)?;
    println!("Processing dictionary");
    process_dictionary()?;

    println!("debug");
    Ok(())
}

