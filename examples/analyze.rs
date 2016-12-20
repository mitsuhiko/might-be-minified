extern crate might_be_minified;

use std::fs;
use std::env;
use might_be_minified::analyze;


pub fn main() {
    let args : Vec<_> = env::args().collect();
    println!("Analyzing {:?}", args[1]);
    let mut f = fs::File::open(&args[1]).unwrap();
    let a = analyze(&mut f);

    println!("results:");
    println!("  space to code: {}", a.space_to_code_ratio());
    println!("  ident median: {}", a.median_ident_length());
    println!("  shape: {}", a.shape());
    println!("  longest line: {}", a.longest_line());
    println!("  p: {}", a.minified_probability());
    println!("");

    if a.is_likely_minified() {
        println!("Minified");
    } else {
        println!("Not Minified");
    }
}
