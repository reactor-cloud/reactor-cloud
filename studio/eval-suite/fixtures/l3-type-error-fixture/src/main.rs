fn main() {
    let name: String = get_name();
    println!("Hello, {}!", name);
}

fn get_name() -> String {
    "World" // Type error: expected String, found &str
}
