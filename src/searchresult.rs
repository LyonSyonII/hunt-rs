pub type Path = thin_str::ThinStr;

pub enum SearchResult {
    Contains(Path),
    Exact(Path),
}

impl SearchResult {
    pub fn contains(path: String) -> Self {
        Self::Contains(path.into())
    }
    pub fn exact(path: String) -> Self {
        Self::Exact(path.into())
    }
}

impl std::fmt::Display for SearchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contains(path) => f.write_str(path),
            Self::Exact(path) => f.write_str(path),
        }
    }
}
