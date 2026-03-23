use anyhow::Error;

fn classify_error(message: &str) -> &'static str {
    let lowered = message.to_ascii_lowercase();
    if lowered.contains("not found") {
        return "not_found";
    }
    if lowered.contains("invalid") || lowered.contains("expected") {
        return "validation";
    }
    if lowered.contains("timed out") {
        return "timeout";
    }
    if lowered.contains("provider") {
        return "provider";
    }
    if lowered.contains("sqlite") || lowered.contains("database") || lowered.contains("db") {
        return "storage";
    }
    "unknown"
}

fn flatten_causes(err: &Error) -> Vec<String> {
    err.chain().skip(1).map(ToString::to_string).collect()
}

fn render_error(err: &Error) -> String {
    let message = err.to_string();
    let category = classify_error(&message);
    let causes = flatten_causes(err);
    serde_json::json!({
        "category": category,
        "message": message,
        "causes": causes,
    })
    .to_string()
}

#[tokio::main]
async fn main() {
    if let Err(err) = agentd::cli::run().await {
        eprintln!("{}", render_error(&err));
        std::process::exit(1);
    }
}
