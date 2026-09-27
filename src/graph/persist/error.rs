use std::fmt;
use std::io;
use std::path::PathBuf;

use super::SectionTag;
use crate::graph::index::Cardinality;

#[derive(Debug)]
pub struct Error {
    pub path: PathBuf,
    pub reason: Reason,
}

#[derive(Debug)]
pub enum Reason {
    Io(io::Error),
    Format(Format),
    EdgeKind { file: u8, expected: u8 },
    Layout(Layout),
    Catalogue(Catalogue),
    Value { site: Site, source: bincode::ErrorKind },
    Promotion(Promotion),
    V2(V2),
}

#[derive(Debug)]
pub enum Format {
    Magic,
    Version { found: u16, expected: u16 },
    SectionTag(u8),
    MissingSection(SectionTag),
    SectionRange(SectionTag),
    Truncated,
    TrailerHash,
    Cardinality(u8),
    Utf8(std::str::Utf8Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Node,
    Edge,
}

#[derive(Debug)]
pub struct Layout {
    pub side: Side,
    pub file_type: String,
    pub expected_type: &'static str,
}

#[derive(Debug)]
pub enum Catalogue {
    Undeclared(Vec<String>),
    Unheld(Vec<String>),
    Mismatch {
        name: String,
        file_cardinality: Cardinality,
        file_tag: u64,
        declared_cardinality: Cardinality,
        declared_tag: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Site {
    Node(u32),
    Edge(u32),
    Free,
    Index(String),
}

#[derive(Debug)]
pub enum Promotion {
    Unrelated { file_type: String, composite: &'static str, part: &'static str },
    Ambiguous { file_type: String, composite: &'static str, part: &'static str },
}

#[derive(Debug)]
pub enum V2 {
    Unconverted,
    Body(bincode::ErrorKind),
}

pub(crate) fn at<T>(path: &std::path::Path, body: impl FnOnce() -> Result<T, Reason>) -> Result<T, Error> {
    body().map_err(|reason| Error { path: path.to_path_buf(), reason })
}

impl Reason {
    pub(crate) fn value(site: Site) -> impl FnOnce(bincode::Error) -> Reason {
        move |source| Reason::Value { site, source: *source }
    }
}

impl From<io::Error> for Reason {
    fn from(e: io::Error) -> Self {
        Reason::Io(e)
    }
}

impl From<Format> for Reason {
    fn from(f: Format) -> Self {
        Reason::Format(f)
    }
}

impl From<Layout> for Reason {
    fn from(l: Layout) -> Self {
        Reason::Layout(l)
    }
}

impl From<Catalogue> for Reason {
    fn from(c: Catalogue) -> Self {
        Reason::Catalogue(c)
    }
}

impl From<Promotion> for Reason {
    fn from(p: Promotion) -> Self {
        Reason::Promotion(p)
    }
}

impl From<V2> for Reason {
    fn from(v: V2) -> Self {
        Reason::V2(v)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.reason)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.reason {
            Reason::Io(e) => Some(e),
            Reason::Value { source, .. } => Some(source),
            Reason::V2(V2::Body(source)) => Some(source),
            Reason::Format(Format::Utf8(e)) => Some(e),
            Reason::Format(_)
            | Reason::EdgeKind { .. }
            | Reason::Layout(_)
            | Reason::Catalogue(_)
            | Reason::Promotion(_)
            | Reason::V2(V2::Unconverted) => None,
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reason::Io(e) => write!(f, "{e}"),
            Reason::Format(format) => write!(f, "{format}"),
            Reason::EdgeKind { file, expected } => write!(f, "edge kind mismatch: file has {file}, expected {expected}"),
            Reason::Layout(layout) => write!(f, "{layout}"),
            Reason::Catalogue(catalogue) => write!(f, "{catalogue}"),
            Reason::Value { site, source } => write!(f, "{site}: {source}"),
            Reason::Promotion(promotion) => write!(f, "{promotion}"),
            Reason::V2(v2) => write!(f, "{v2}"),
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Format::Magic => write!(f, "invalid magic"),
            Format::Version { found, expected } => write!(f, "unsupported version: {found} (expected {expected})"),
            Format::SectionTag(byte) => write!(f, "unknown section tag {byte}"),
            Format::MissingSection(tag) => write!(f, "missing section {tag:?}"),
            Format::SectionRange(tag) => write!(f, "section {tag:?} range out of bounds"),
            Format::Truncated => write!(f, "file truncated"),
            Format::TrailerHash => write!(f, "trailer hash mismatch: file is corrupted"),
            Format::Cardinality(byte) => write!(f, "unknown index cardinality {byte}"),
            Format::Utf8(e) => write!(f, "invalid UTF-8: {e}"),
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Side::Node => write!(f, "node"),
            Side::Edge => write!(f, "edge"),
        }
    }
}

impl fmt::Display for Layout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} value layout mismatch: file has type `{}`, expected `{}`",
            self.side, self.file_type, self.expected_type
        )
    }
}

impl fmt::Display for Catalogue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Catalogue::Undeclared(names) => write!(f, "missing declarations for indices: {}", names.join(", ")),
            Catalogue::Unheld(names) => {
                write!(f, "declarations for indices not present in the file: {}", names.join(", "))
            }
            Catalogue::Mismatch { name, file_cardinality, file_tag, declared_cardinality, declared_tag } => write!(
                f,
                "index `{name}` declaration mismatch: file has cardinality {file_cardinality:?} tag {file_tag}, declared cardinality {declared_cardinality:?} tag {declared_tag}"
            ),
        }
    }
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Site::Node(slot) => write!(f, "value of node {slot}"),
            Site::Edge(slot) => write!(f, "value of edge {slot}"),
            Site::Free => write!(f, "free section"),
            Site::Index(name) => write!(f, "id set of index `{name}`"),
        }
    }
}

impl fmt::Display for Promotion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Promotion::Unrelated { file_type, composite, part } => write!(
                f,
                "edge value layout mismatch: file has type `{file_type}`, expected `{composite}` or its part `{part}`"
            ),
            Promotion::Ambiguous { file_type, composite, part } => write!(
                f,
                "edge value layout ambiguous: file has type `{file_type}`, and `{composite}` shares its layout hash with its part `{part}`"
            ),
        }
    }
}

impl fmt::Display for V2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            V2::Unconverted => write!(
                f,
                "unsupported version: 2 (expected 3); this is a v2 file, convert it first with `grw::graph::persist::convert`"
            ),
            V2::Body(source) => write!(f, "v2 body: {source}"),
        }
    }
}
