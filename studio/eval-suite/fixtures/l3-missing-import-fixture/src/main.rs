fn main() {
    let mut cache: HashMap<String, i32> = HashMap::new();
    cache.insert("one".to_string(), 1);
    cache.insert("two".to_string(), 2);
    
    for (key, value) in &cache {
        println!("{}: {}", key, value);
    }
}
