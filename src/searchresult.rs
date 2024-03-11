#[repr(transparent)]
pub struct SearchResult {
    path: thin_str::ThinStr,
}

impl SearchResult {
    pub fn contains(s: impl Into<thin_str::ThinStr>) -> Self {
        SearchResult {
            path: s.into(),
        }
    }
    pub fn exact(mut p: String) -> Self {
        p.push('\0');
        SearchResult {
            path: p.into(),
        }
    }
    pub fn is_exact(&self) -> bool {
        self.path.ends_with('\0')
    }
    pub fn into_path(self) -> thin_str::ThinStr {
        self.path
    }
}

impl std::fmt::Display for SearchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.path)
    }
}