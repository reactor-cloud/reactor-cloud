fn main() {
    let input = "hello,world,test";
    match process_data(input) {
        Ok(result) => println!("Result: {:?}", result),
        Err(e) => eprintln!("Error: {}", e),
    }
}

fn process_data(input: &str) -> Result<Vec<String>, String> {
    // Validation logic - should be extracted
    if input.is_empty() {
        return Err("Input cannot be empty".to_string());
    }
    if !input.contains(',') {
        return Err("Input must contain comma-separated values".to_string());
    }
    if input.len() > 1000 {
        return Err("Input too long".to_string());
    }
    
    // Transformation logic - should be extracted
    let parts: Vec<&str> = input.split(',').collect();
    let mut result = Vec::new();
    for part in parts {
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            result.push(trimmed.to_uppercase());
        }
    }
    
    Ok(result)
}
