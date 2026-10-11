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
    ///
    /// The string is a file path or a `file://` URI. A URI is associated with
    /// the path it names.
    pub fn get_path(s: &str) -> PathBuf {
        let path = match s.strip_prefix(FILE_URI_SCHEME) {
            Some(uri) => file_uri_path(uri),
            None => PathBuf::from(s),
        };
        let absolute = if path.is_absolute() {
            path
        } else {
            match std::env::current_dir() {
                Ok(cwd) => cwd.join(path),
                Err(_) => path,
            }
        };
        normalize(&absolute)
    }

    /// Construct a file from a string representing a file path
    pub fn from_string(s: &str) -> File {
        File::Path(File::get_path(s))
    }
}

/// The scheme of a `file://` URI
///
/// `fpp_lsp_server` names each source file by the URI the editor knows it by,
/// where the command-line tools name it by its path.
const FILE_URI_SCHEME: &str = "file://";

/// Get the path that a `file://` URI names, given the part of the URI after
/// the scheme: an authority, which is empty or `localhost` for a file on this
/// host, followed by the percent-encoded absolute path
fn file_uri_path(uri: &str) -> PathBuf {
    let (authority, path) = uri.split_at(uri.find('/').unwrap_or(uri.len()));
    let path = percent_encoding::percent_decode_str(path).decode_utf8_lossy();
    match authority {
        "" | "localhost" => {}
        // A file on another host, as a UNC path
        host => return PathBuf::from(format!("//{host}{path}")),
    }
    // A Windows path starts with its drive letter: `/C:/a` names `C:/a`
    if cfg!(windows)
        && let Some(drive_path) = path.strip_prefix('/')
        && drive_path.as_bytes().get(1) == Some(&b':')
    {
        return PathBuf::from(drive_path);
    }
    PathBuf::from(path.into_owned())
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
    ///
    /// The result is a path even when `current` is named by a `file://` URI, so
    /// a reader that names its files by URI resolves the includes itself.
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
    /// For standard input, this is the current working directory. For a file
    /// named by a `file://` URI, it is the directory of the path the URI names.
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

#[cfg(test)]
mod tests {
    use crate::{
        CompilerContext, DiagnosticData, DiagnosticEmitter, Error, File, FileReader, SourceFile,
    };
    use std::path::PathBuf;

    struct NoEmitter;

    impl DiagnosticEmitter for NoEmitter {
        fn emit(&mut self, _diagnostic: DiagnosticData) {}
    }

    /// A reader that resolves paths with the default implementation
    struct DefaultReader;

    impl FileReader for DefaultReader {
        fn read(&self, path: &str) -> Result<String, Error> {
            Err(format!("cannot read {path}").into())
        }
    }

    #[test]
    fn resolve_is_relative_to_the_directory_and_lexical() {
        crate::run(&mut CompilerContext::new(NoEmitter), || {
            let file = SourceFile::new("/a/b/c.fpp", String::new());
            let resolve = |include| DefaultReader.resolve(file, include).unwrap();
            assert_eq!(resolve("d.fppi"), "/a/b/d.fppi");
            assert_eq!(resolve("./nonexistent/../d.fppi"), "/a/b/d.fppi");
            assert_eq!(resolve("../../e.fppi"), "/e.fppi");

            let stdin = SourceFile::new("<stdin>", String::new());
            let cwd = std::env::current_dir().unwrap();
            assert_eq!(
                DefaultReader.resolve(stdin, "f.fppi").unwrap(),
                cwd.join("f.fppi").to_string_lossy()
            );
        });
    }

    #[test]
    fn get_path_of_a_file_uri_is_the_path_it_names() {
        assert_eq!(
            File::get_path("file:///a/b/../c.fpp"),
            PathBuf::from("/a/c.fpp")
        );
        assert_eq!(
            File::get_path("file:///a/my%20dir/%C3%A9.fpp"),
            PathBuf::from("/a/my dir/\u{e9}.fpp")
        );
        assert_eq!(
            File::get_path("file://localhost/a/c.fpp"),
            PathBuf::from("/a/c.fpp")
        );
        assert_eq!(File::get_path("file:///../c.fpp"), PathBuf::from("/c.fpp"));
        assert_eq!(
            File::from_string("file:///a/./c.fpp").to_string(),
            "/a/c.fpp"
        );
    }

    #[test]
    fn paths_relative_to_a_file_uri_are_paths() {
        crate::run(&mut CompilerContext::new(NoEmitter), || {
            let file = SourceFile::new("file:///a/my%20dir/c.fpp", String::new());
            assert_eq!(file.get_dir_path(), PathBuf::from("/a/my dir"));
            assert_eq!(
                file.get_relative_path("d.fpp"),
                PathBuf::from("/a/my dir/d.fpp")
            );
            assert_eq!(
                file.get_relative_path("./sub/../../e.fpp"),
                PathBuf::from("/a/e.fpp")
            );
            // A `..` above the root stays at the root, as it does on the path
            // side of a comparison
            assert_eq!(
                file.get_relative_path("../../../f.fpp"),
                PathBuf::from("/f.fpp")
            );
            assert_eq!(
                DefaultReader.resolve(file, "g.fppi").unwrap(),
                "/a/my dir/g.fppi"
            );
        });
    }

    /// A location specifier names a file by a path relative to the specifier,
    /// and the definition's file is named by its URI. Both sides name the same
    /// file once they are normalized.
    #[test]
    fn relative_path_matches_the_file_uri_it_names() {
        crate::run(&mut CompilerContext::new(NoEmitter), || {
            let specifier = SourceFile::new("file:///p/locs/my%20locs.fpp", String::new());
            let actual = SourceFile::new("file:///p/Fw/Time%20Defs.fpp", String::new());
            assert_eq!(
                File::Path(specifier.get_relative_path("../Fw/Time Defs.fpp")).to_string(),
                File::from_string(&actual.uri()).to_string()
            );
            assert_eq!(
                File::from_string(&actual.uri()).to_string(),
                "/p/Fw/Time Defs.fpp"
            );
        });
    }
}
