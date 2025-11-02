use std::collections::HashMap;

fn get_formatter() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("Personal Best", "PB"),
        ("Balanced PB", "Balanced"),
        ("Best Segments", "SOB"),
        ("Best Split Times", "Best Split"),
        ("Average Segments", "Avg"),
        ("Median Segments", "Median"),
        ("Worst Segments", "Worst Split"),
        ("Latest Run", "Latest"),
    ])
}

pub fn format_label(input: &str) -> &str {
    get_formatter().get(input).copied().unwrap_or(input)
}
