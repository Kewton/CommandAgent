fn main() {
    let input = std::env::args().nth(1).unwrap_or_else(|| "world".to_string());
    println!("hello {input}");
}
