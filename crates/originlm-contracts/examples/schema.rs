fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&originlm_contracts::schema_document()).unwrap()
    );
}
