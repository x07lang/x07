pub fn validate_module_id(module_id: &str) -> Result<(), String> {
    let module_id = module_id.trim();
    if module_id.is_empty() {
        return Err("module_id must not be empty".to_string());
    }
    if module_id.contains('/') || module_id.contains('\\') {
        return Err(format!(
            "invalid module_id (path separators are not allowed): {module_id:?}"
        ));
    }

    for seg in module_id.split('.') {
        if seg.is_empty() {
            return Err(format!("invalid module_id (empty segment): {module_id:?}"));
        }
        if seg == "." || seg == ".." {
            return Err(format!(
                "invalid module_id (dot segments are not allowed): {module_id:?}"
            ));
        }

        let mut chars = seg.chars();
        let first = chars.next().unwrap_or('_');
        if !(first.is_ascii_alphabetic() || first == '_') {
            return Err(format!(
                "invalid module_id segment start (must be [A-Za-z_]): {module_id:?} segment={seg:?}"
            ));
        }
        for c in chars {
            if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return Err(format!(
                    "invalid module_id segment char (allowed [A-Za-z0-9_-]): {module_id:?} segment={seg:?}"
                ));
            }
        }
    }

    Ok(())
}

pub fn validate_symbol(sym: &str) -> Result<(), String> {
    validate_module_id(sym)
}

pub fn validate_local_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("local name must be non-empty".to_string());
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap_or('_');
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "invalid local name start (must be [A-Za-z_]): {name:?}"
        ));
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!(
                "invalid local name char (allowed [A-Za-z0-9_]): {name:?}"
            ));
        }
    }
    Ok(())
}

pub fn validate_type_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("type name must be non-empty".to_string());
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap_or('_');
    if !first.is_ascii_alphabetic() {
        return Err(format!(
            "invalid type name start (must be [A-Za-z]): {name:?}"
        ));
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!(
                "invalid type name char (allowed [A-Za-z0-9_]): {name:?}"
            ));
        }
    }
    Ok(())
}
