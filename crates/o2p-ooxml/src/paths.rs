use crate::{PackageError, Result};

fn safe_uri_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.starts_with('/')
        || path
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | ':' | '?' | '#' | ' '))
    {
        return Err(PackageError::InvalidPath(path.into()));
    }
    // Keep escaped names stable; reject encoded separators/dots and double decoding.
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let code = bytes
                .get(i + 1..i + 3)
                .and_then(|s| std::str::from_utf8(s).ok())
                .and_then(|s| u8::from_str_radix(s, 16).ok())
                .ok_or_else(|| PackageError::InvalidPath(path.into()))?;
            if code <= 0x20 || matches!(code, b'/' | b'\\' | b'.' | b':' | b'?' | b'#' | b'%') {
                return Err(PackageError::InvalidPath(path.into()));
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// Canonicalize relative package names. ZIP/member/lookup paths cannot contain `..`.
/// Benign `.` segments are removed; empty segments and absolute/URI paths are errors.
pub fn normalize_part_path(path: &str) -> Result<String> {
    safe_uri_path(path)?;
    let mut segments = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | ".." => return Err(PackageError::InvalidPath(path.into())),
            "." => (),
            segment => segments.push(segment),
        }
    }
    if segments.is_empty() {
        return Err(PackageError::InvalidPath(path.into()));
    }
    Ok(segments.join("/"))
}

/// Resolve an internal URI against a source part's directory (`None` = package root).
/// Parent references are valid only when they remain inside the package root.
pub fn resolve_internal_target(source: Option<&str>, target: &str) -> Result<String> {
    safe_uri_path(target)?;
    let source = source.map(normalize_part_path).transpose()?;
    let mut segments: Vec<&str> = source
        .as_deref()
        .and_then(|s| s.rsplit_once('/'))
        .map_or_else(Vec::new, |(directory, _)| directory.split('/').collect());
    for segment in target.split('/') {
        match segment {
            "" => return Err(PackageError::InvalidPath(target.into())),
            "." => (),
            ".." => {
                if segments.pop().is_none() {
                    return Err(PackageError::InvalidPath(target.into()));
                }
            }
            segment => segments.push(segment),
        }
    }
    normalize_part_path(&segments.join("/"))
}

pub(crate) fn relationship_source(name: &str) -> Result<Option<String>> {
    if name == "_rels/.rels" {
        return Ok(Some(String::new()));
    }
    if !name.ends_with(".rels") {
        return Ok(None);
    }
    let (directory, file) = name.rsplit_once('/').ok_or_else(|| {
        PackageError::InvalidMetadata(format!("Invalid relationship part {name}"))
    })?;
    let prefix = if directory == "_rels" {
        ""
    } else {
        directory.strip_suffix("/_rels").ok_or_else(|| {
            PackageError::InvalidMetadata(format!("Invalid relationship part {name}"))
        })?
    };
    let file = file
        .strip_suffix(".rels")
        .ok_or_else(|| PackageError::InvalidPath(name.into()))?;
    let source = if prefix.is_empty() {
        file.into()
    } else {
        format!("{prefix}/{file}")
    };
    Ok(Some(normalize_part_path(&source)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_and_relative_targets() {
        assert_eq!(
            normalize_part_path("ppt/./slides/slide1.xml").unwrap(),
            "ppt/slides/slide1.xml"
        );
        assert_eq!(
            resolve_internal_target(Some("ppt/slides/slide1.xml"), "../media/image1.png").unwrap(),
            "ppt/media/image1.png"
        );
        assert_eq!(
            resolve_internal_target(None, "./ppt/presentation.xml").unwrap(),
            "ppt/presentation.xml"
        );
        assert_eq!(
            relationship_source("ppt/slides/_rels/slide1.xml.rels").unwrap(),
            Some("ppt/slides/slide1.xml".into())
        );
        assert_eq!(
            relationship_source("_rels/document.xml.rels").unwrap(),
            Some("document.xml".into())
        );
    }
    #[test]
    fn invalid_paths() {
        for name in [
            "",
            ".",
            "/ppt/a.xml",
            "C:/a",
            "//host/a",
            "ppt\\a",
            "../a",
            "ppt/../a",
            "ppt//a",
            "ppt/a/",
            "ppt/%2e%2e/a",
            "ppt/%2Ffoo",
            "ppt/%25foo",
            "ppt/%00",
            "ppt/%zz",
            "a?b",
            "a#b",
            "a\0b",
        ] {
            assert!(normalize_part_path(name).is_err(), "{name:?}");
        }
        for target in [
            "../../../escape",
            "/absolute",
            "https://example.com",
            "..",
            "%2e%2e/a",
        ] {
            assert!(resolve_internal_target(Some("ppt/slide.xml"), target).is_err());
        }
    }
}
