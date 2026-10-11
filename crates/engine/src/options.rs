#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OptionKind {
    Check,
    Spin,
    Combo,
    String,
    Button,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct EngineOption {
    pub name: String,
    pub kind: OptionKind,
    pub default: Option<String>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub vars: Vec<String>,
}
pub fn validate_option(option: &EngineOption, value: &str) -> Result<String, crate::EngineError> {
    use crate::EngineError;
    let error = || EngineError::Option(format!("{}: invalid value", option.name));
    if option.name.is_empty() || option.name.contains(['\r', '\n']) || value.contains(['\r', '\n'])
    {
        return Err(error());
    }
    match option.kind {
        OptionKind::Check if !matches!(value, "true" | "false") => return Err(error()),
        OptionKind::Spin => {
            let n = value.parse::<i64>().map_err(|_| error())?;
            if option.min.is_some_and(|min| n < min) || option.max.is_some_and(|max| n > max) {
                return Err(error());
            }
        }
        OptionKind::Combo if !option.vars.iter().any(|v| v == value) => return Err(error()),
        OptionKind::Button if !value.is_empty() => return Err(error()),
        _ => {}
    }
    let command = if option.kind == OptionKind::Button {
        format!("setoption name {}", option.name)
    } else {
        format!("setoption name {} value {value}", option.name)
    };
    if command.len() > 16384 {
        return Err(EngineError::LineLimit);
    }
    Ok(command)
}

pub(crate) fn parse_option(line: &str) -> Result<EngineOption, crate::EngineError> {
    use crate::EngineError;
    let tokens: Vec<_> = line.split_whitespace().collect();
    let error = || EngineError::Protocol("Malformed option declaration".into());
    if tokens.get(1) != Some(&"name") {
        return Err(error());
    }
    let kind_at = tokens.iter().position(|&s| s == "type").ok_or_else(error)?;
    if kind_at <= 2 {
        return Err(error());
    }
    let kind = match tokens.get(kind_at + 1) {
        Some(&"check") => OptionKind::Check,
        Some(&"spin") => OptionKind::Spin,
        Some(&"combo") => OptionKind::Combo,
        Some(&"string") => OptionKind::String,
        Some(&"button") => OptionKind::Button,
        _ => return Err(error()),
    };
    // The slices come from this line; retain interior whitespace in names and paths.
    let value = |start: usize, end: usize| -> String {
        if start >= end {
            return String::new();
        }
        let first = tokens[start].as_ptr() as usize - line.as_ptr() as usize;
        let last =
            tokens[end - 1].as_ptr() as usize - line.as_ptr() as usize + tokens[end - 1].len();
        line[first..last].to_owned()
    };
    let mut option = EngineOption {
        name: value(2, kind_at),
        kind,
        default: None,
        min: None,
        max: None,
        vars: vec![],
    };
    let mut i = kind_at + 2;
    if option.kind == OptionKind::String {
        if tokens.get(i) == Some(&"default") {
            option.default = Some(value(i + 1, tokens.len()));
        } else if i != tokens.len() {
            return Err(error());
        }
    } else {
        while i < tokens.len() {
            let field = tokens[i];
            i += 1;
            let begin = i;
            while i < tokens.len() && !matches!(tokens[i], "default" | "min" | "max" | "var") {
                i += 1;
            }
            let text = value(begin, i);
            match field {
                "default" => option.default = Some(text),
                "var" if option.kind == OptionKind::Combo && !text.is_empty() => {
                    option.vars.push(text)
                }
                "min" if option.kind == OptionKind::Spin => {
                    option.min = Some(text.parse().map_err(|_| error())?)
                }
                "max" if option.kind == OptionKind::Spin => {
                    option.max = Some(text.parse().map_err(|_| error())?)
                }
                _ => return Err(error()),
            }
        }
    }
    if option.default.as_deref() == Some("<empty>") {
        option.default = Some(String::new());
    }
    if option
        .min
        .zip(option.max)
        .is_some_and(|(min, max)| min > max)
    {
        return Err(error());
    }
    if option.kind == OptionKind::Combo && option.vars.is_empty() {
        return Err(error());
    }
    if let Some(default) = &option.default {
        validate_option(&option, default).map_err(|_| error())?;
    }
    Ok(option)
}
