use std::collections::BTreeMap;

pub(crate) fn kebab(value: &str) -> Result<String, String> {
    normalized(value, '-')
}

pub(crate) fn snake(value: &str) -> Result<String, String> {
    normalized(value, '_')
}

pub(crate) fn pascal(value: &str) -> Result<String, String> {
    let snake = snake(value)?;
    Ok(snake
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect())
}

pub(crate) fn type_parameter(value: &str) -> Result<String, String> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err("type parameter must not be empty".to_owned());
    };
    if !(first.is_ascii_alphabetic() || first == '_')
        || !chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(format!("invalid Interface IR type parameter {value:?}"));
    }
    Ok(value.to_owned())
}

pub(crate) fn insert_unique(
    seen: &mut BTreeMap<String, String>,
    generated: &str,
    owner: &str,
    target: &str,
) -> Result<(), String> {
    if let Some(previous) = seen.insert(generated.to_owned(), owner.to_owned()) {
        return Err(format!(
            "generated {target} identifier {generated:?} collides between {previous} and {owner}"
        ));
    }
    Ok(())
}

fn normalized(value: &str, separator: char) -> Result<String, String> {
    let mut output = String::new();
    let mut previous_was_separator = false;
    let mut previous_was_lower_or_digit = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_uppercase()
                && previous_was_lower_or_digit
                && !previous_was_separator
            {
                output.push(separator);
            }
            output.push(character.to_ascii_lowercase());
            previous_was_separator = false;
            previous_was_lower_or_digit =
                character.is_ascii_lowercase() || character.is_ascii_digit();
        } else {
            if !output.is_empty() && !previous_was_separator {
                output.push(separator);
            }
            previous_was_separator = true;
            previous_was_lower_or_digit = false;
        }
    }
    while output.ends_with(separator) {
        output.pop();
    }
    if output.is_empty() {
        return Err(format!("cannot derive an identifier from {value:?}"));
    }
    if output.starts_with(|character: char| character.is_ascii_digit()) {
        output = format!("ffi{separator}{output}");
    }
    Ok(output)
}
