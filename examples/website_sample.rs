use ferroni::prelude::*;

fn main() -> Result<(), RegexError> {
    let regex = Regex::new(r"(?<=Date: )(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})")?;
    let text = "Date: 2026-09-26";
    let captures = regex.captures(text).expect("the sample date matches");

    println!("input: {text}");
    for name in ["year", "month", "day"] {
        let value = captures.name(name).expect("the named group exists");
        println!("{name}: {}", value.as_str());
    }
    Ok(())
}
