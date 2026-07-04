fn main() {
    let src = std::env::args().nth(1).expect("need source");
    let result = zenlang::formatter::format_source(&src, 4).unwrap();
    println!("{}", result);
}
