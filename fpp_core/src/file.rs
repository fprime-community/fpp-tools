use line_index::LineIndex;

use crate::interface::with;
use crate::{Error, Span};
use std::cell::Ref;
use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};

/// A file used in compilation
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum File {
    /// A file path
    Path(PathBuf),
    /// Standard input
    StdIn,
}

impl Display for File {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            File::Path(p) => f.write_str(&normalize(p).to_string_lossy()),
            File::StdIn => f.write_str("stdin"),
        }
    }
}

impl File {
    /// Get the absolute, normalized path associated with a string
    pub fn get_path(s: &str) -> PathBuf {
        let path = Path::new(s);
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            match std::env::current_dir() {
                Ok(cwd) => cwd.join(path),
                Err(_) => path.to_path_buf(),
            }
        };
        normalize(&absolute)
    }

    /// Construct a file from a string representing a file path
    pub fn from_string(s: &str) -> File {
        File::Path(File::get_path(s))
    }
}

/// Collapse `.` and `..` components in a path, without touching the file system
fn normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(Component::ParentDir);
                }
            }
            other => out.push(other),
        }
    }
    out
}

pub trait FileReader {
    /// Resolve an include path relative to the parent source file
    /// where the "include" exists
    fn resolve(&self, current: SourceFile, include: &str) -> Result<String, Error> {
        let current_path = if current.uri() == "<stdin>" {
            // Read from relative to the current directory
            return Ok(include.to_string());
        } else {
            current.uri()
        };

        let parent_file_path = std::path::Path::new(&current_path).canonicalize()?;
        match parent_file_path.parent() {
            None => Err(format!("Cannot resolve parent directory of {}", current_path).into()),
            Some(parent_dir) => {
                let final_path = parent_dir.join(include);
                match final_path.as_path().to_str() {
                    None => Err(format!(
                        "Failed to resolve path {} relative to {:?}",
                        include, parent_dir
                    )
                    .into()),
                    Some(file_path) => Ok(file_path.to_string()),
                }
            }
        }
    }

    /// Read a file given its path
    fn read(&self, path: &str) -> Result<String, Error>;
}

pub type BytePos = u32;

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub struct SourceFile {
    pub(crate) handle: usize,
}

pub struct SourceFileContent<'a> {
    data: Ref<'a, String>,
}

impl<'a> AsRef<str> for SourceFileContent<'a> {
    fn as_ref(&self) -> &str {
        self.data.as_str()
    }
}

impl Debug for SourceFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("SourceFile {{ {} }}", self.handle))
    }
}

impl Display for SourceFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.uri())
    }
}

impl SourceFile {
    /// Create a new source file given a unique Universal-Resource-Identifier (URI) and it's contents.
    /// The URI is used by a [FileReader] to identify the source location.
    /// If there already is a file with this URI in the compiler, that file will be dropped and all
    /// its spans and AST nodes will be dropped.
    ///
    /// # Arguments
    ///
    /// * `uri`: File's unique URI
    /// * `content`: File content
    ///
    /// returns: SourceFile
    pub fn new(uri: &str, content: String) -> SourceFile {
        with(|w| w.file_new(uri, content, None))
    }

    pub fn new_with_parent(uri: &str, content: String, parent: SourceFile) -> SourceFile {
        with(|w| w.file_new(uri, content, Some(parent)))
    }

    pub fn uri(&self) -> String {
        with(|w| w.file_uri(self))
    }

    /// Get the parent file this file was included from (if any)
    pub fn parent(&self) -> Option<SourceFile> {
        with(|w| w.file_parent(self))
    }

    pub fn read(&self) -> SourceFileContent<'_> {
        with(|w| SourceFileContent {
            data: w.file_content(self),
        })
    }

    pub fn lines(&self) -> Ref<'_, LineIndex> {
        with(|w| w.file_lines(self))
    }

    pub fn read_snippet(&self, span: &Span) -> String {
        let start = span.start();
        let end = span.end();

        with(|w| {
            let content = &w.file_content(self);
            content.as_str()[((start.pos - start.column) as usize)..(end.pos as usize)].to_string()
        })
    }

    pub fn len(&self) -> usize {
        with(|w| w.file_len(self))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
