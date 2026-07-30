use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt;
use thiserror::Error;

pub const MAX_SAFE_COMPONENT_UTF16: usize = 255;
pub const MAX_SAFE_PATH_COMPONENTS: usize = 256;
pub const MAX_SAFE_PATH_UTF16: usize = 32_000;

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PathSafetyError {
    #[error("a recovery path must contain at least one component")]
    EmptyPath,
    #[error("path depth {actual} exceeds the limit of {maximum} components")]
    ExcessiveDepth { actual: usize, maximum: usize },
    #[error("path length {actual} exceeds the limit of {maximum} UTF-16 code units")]
    ExcessivePathLength { actual: usize, maximum: usize },
    #[error("component {index} is empty")]
    EmptyComponent { index: usize },
    #[error("component {index} is a traversal component")]
    Traversal { index: usize },
    #[error("component {index} contains a path separator or Windows prefix")]
    PathInjection { index: usize },
    #[error("component {index} contains a Windows-forbidden character")]
    ForbiddenCharacter { index: usize },
    #[error("component {index} contains a control character")]
    ControlCharacter { index: usize },
    #[error("component {index} has a trailing dot or space")]
    TrailingDotOrSpace { index: usize },
    #[error("component {index} is a reserved Windows device name")]
    ReservedWindowsName { index: usize },
    #[error("component {index} is {actual} UTF-16 code units; the maximum is {maximum}")]
    ExcessiveComponentLength {
        index: usize,
        actual: usize,
        maximum: usize,
    },
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct SafeComponent(String);

impl SafeComponent {
    fn parse(value: &str, index: usize) -> Result<Self, PathSafetyError> {
        validate_component(value, index)?;
        Ok(Self(value.to_owned()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SafeComponent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SafeComponent")
            .field(&self.0)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SafeRelativePath {
    components: Vec<SafeComponent>,
}

impl SafeRelativePath {
    pub fn try_from_components<I, S>(components: I) -> Result<Self, PathSafetyError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let raw = components.into_iter().collect::<Vec<_>>();
        validate_depth(raw.len())?;
        let mut parsed = Vec::with_capacity(raw.len());
        let mut total_utf16 = 0usize;
        for (index, component) in raw.iter().enumerate() {
            let safe = SafeComponent::parse(component.as_ref(), index)?;
            total_utf16 =
                checked_path_length(total_utf16, safe.as_str().encode_utf16().count(), index)?;
            parsed.push(safe);
        }
        Ok(Self { components: parsed })
    }

    pub fn derive_for_recovery<S: AsRef<str>>(
        recovered_parents: &[S],
        recovered_name: &str,
    ) -> Result<DerivedSafePath, PathSafetyError> {
        let component_count =
            recovered_parents
                .len()
                .checked_add(1)
                .ok_or(PathSafetyError::ExcessiveDepth {
                    actual: usize::MAX,
                    maximum: MAX_SAFE_PATH_COMPONENTS,
                })?;
        validate_depth(component_count)?;

        let mut safe_components = Vec::with_capacity(component_count);
        let mut substitutions = Vec::new();
        let mut total_utf16 = 0usize;

        for (index, original) in recovered_parents
            .iter()
            .map(AsRef::as_ref)
            .chain(std::iter::once(recovered_name))
            .enumerate()
        {
            let (component, reason) = match SafeComponent::parse(original, index) {
                Ok(component) => (component, None),
                Err(error) => {
                    let replacement = fallback_component(original);
                    (
                        SafeComponent::parse(&replacement, index)
                            .expect("fixed hash fallback is always a safe component"),
                        Some(error),
                    )
                }
            };
            total_utf16 = checked_path_length(
                total_utf16,
                component.as_str().encode_utf16().count(),
                index,
            )?;
            if let Some(reason) = reason {
                substitutions.push(PathSubstitution {
                    component_index: index,
                    original: original.to_owned(),
                    replacement: component.as_str().to_owned(),
                    reason,
                });
            }
            safe_components.push(component);
        }

        let evidence_json = serde_json::to_string(&PathEvidence {
            version: 1,
            substitutions,
        })
        .expect("bounded path evidence is serializable");
        Ok(DerivedSafePath {
            path: Self {
                components: safe_components,
            },
            evidence_json,
        })
    }

    pub fn components(&self) -> impl ExactSizeIterator<Item = &str> {
        self.components.iter().map(SafeComponent::as_str)
    }

    pub fn to_slash_string(&self) -> String {
        self.components().collect::<Vec<_>>().join("/")
    }

    pub(crate) fn file_name(&self) -> &str {
        self.components
            .last()
            .expect("safe relative paths are non-empty")
            .as_str()
    }

    pub(crate) fn parent_components(&self) -> impl Iterator<Item = &str> {
        self.components[..self.components.len() - 1]
            .iter()
            .map(SafeComponent::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedSafePath {
    path: SafeRelativePath,
    evidence_json: String,
}

impl DerivedSafePath {
    pub fn path(&self) -> &SafeRelativePath {
        &self.path
    }

    pub fn evidence_json(&self) -> &str {
        &self.evidence_json
    }

    pub(crate) fn into_parts(self) -> (SafeRelativePath, String) {
        (self.path, self.evidence_json)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PathEvidence {
    version: u32,
    substitutions: Vec<PathSubstitution>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PathSubstitution {
    component_index: usize,
    original: String,
    replacement: String,
    reason: PathSafetyError,
}

fn validate_depth(component_count: usize) -> Result<(), PathSafetyError> {
    if component_count == 0 {
        return Err(PathSafetyError::EmptyPath);
    }
    if component_count > MAX_SAFE_PATH_COMPONENTS {
        return Err(PathSafetyError::ExcessiveDepth {
            actual: component_count,
            maximum: MAX_SAFE_PATH_COMPONENTS,
        });
    }
    Ok(())
}

fn checked_path_length(
    current: usize,
    component_len: usize,
    index: usize,
) -> Result<usize, PathSafetyError> {
    let separator = usize::from(index != 0);
    let actual = current
        .checked_add(separator)
        .and_then(|value| value.checked_add(component_len))
        .ok_or(PathSafetyError::ExcessivePathLength {
            actual: usize::MAX,
            maximum: MAX_SAFE_PATH_UTF16,
        })?;
    if actual > MAX_SAFE_PATH_UTF16 {
        return Err(PathSafetyError::ExcessivePathLength {
            actual,
            maximum: MAX_SAFE_PATH_UTF16,
        });
    }
    Ok(actual)
}

fn validate_component(value: &str, index: usize) -> Result<(), PathSafetyError> {
    if value.is_empty() {
        return Err(PathSafetyError::EmptyComponent { index });
    }
    if value == "." || value == ".." {
        return Err(PathSafetyError::Traversal { index });
    }
    if value.contains('/') || value.contains('\\') {
        return Err(PathSafetyError::PathInjection { index });
    }
    if value
        .chars()
        .any(|character| character <= '\u{1f}' || character == '\u{7f}')
    {
        return Err(PathSafetyError::ControlCharacter { index });
    }
    if value
        .chars()
        .any(|character| matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
        return Err(PathSafetyError::ForbiddenCharacter { index });
    }
    if value.ends_with('.') || value.ends_with(' ') {
        return Err(PathSafetyError::TrailingDotOrSpace { index });
    }
    if is_reserved_windows_name(value) {
        return Err(PathSafetyError::ReservedWindowsName { index });
    }
    let actual = value.encode_utf16().count();
    if actual > MAX_SAFE_COMPONENT_UTF16 {
        return Err(PathSafetyError::ExcessiveComponentLength {
            index,
            actual,
            maximum: MAX_SAFE_COMPONENT_UTF16,
        });
    }
    Ok(())
}

fn is_reserved_windows_name(value: &str) -> bool {
    let stem = value.split('.').next().unwrap_or(value).to_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || is_numbered_device(&stem, "COM")
        || is_numbered_device(&stem, "LPT")
}

fn is_numbered_device(stem: &str, prefix: &str) -> bool {
    let Some(suffix) = stem.strip_prefix(prefix) else {
        return false;
    };
    matches!(
        suffix,
        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
    )
}

fn fallback_component(original: &str) -> String {
    let digest = Sha256::digest(original.as_bytes());
    format!("recovered-{}", hex_lower(&digest))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
