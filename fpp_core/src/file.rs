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

/// Collapse `.` and `..` components in a path, without touching the file system,
/// as `java.nio.file.Path.normalize` does: a `..` cancels the name before it, a
/// `..` at the root is the root, and a `..` that has no name before it in a
/// relative path stays
pub(crate) fn normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                Some(Component::ParentDir | Component::CurDir) | None => {
                    out.push(Component::ParentDir);
                }
            },
            other => out.push(other),
        }
    }
    out
}

pub trait FileReader {
    /// Resolve the path of a file included from the source file `current`,
    /// relative to the directory of `current`
    fn resolve(&self, current: SourceFile, include: &str) -> Result<String, Error> {
        Ok(current
            .get_relative_path(include)
            .to_string_lossy()
            .into_owned())
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

    /// Get the directory path associated with the file
    ///
    /// For standard input, this is the current working directory.
    pub fn get_dir_path(&self) -> PathBuf {
        let uri = self.uri();
        if uri == "<stdin>" {
            File::get_path("")
        } else {
            let path = File::get_path(&uri);
            path.parent().map(Path::to_path_buf).unwrap_or(path)
        }
    }

    /// Resolve a path relative to the directory of the file
    ///
    /// The result is normalized lexically, without reading the file system.
    pub fn get_relative_path(&self, path: &str) -> PathBuf {
        normalize(&self.get_dir_path().join(path))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
