use thin_str::ThinStr;

pub enum SearchResult {
    Contains(ThinStr),
    Exact(ThinStr),
}

impl SearchResult {
    #[inline(always)]
    pub fn contains(path: String) -> Self {
        Self::Contains(path.into())
    }
    #[inline(always)]
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

pub struct SearchResults {
    pub exact: Vec<ThinStr>,
    pub contains: Vec<ThinStr>,
}

impl SearchResults {
    pub fn new() -> Self {
        Self {
            exact: Vec::new(),
            contains: Vec::new(),
        }
    }

    #[inline(always)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            exact: Vec::with_capacity(capacity),
            contains: Vec::with_capacity(capacity),
        }
    }

    #[inline(always)]
    pub fn push(&mut self, result: SearchResult) {
        match result {
            SearchResult::Contains(r) => self.contains.push(r),
            SearchResult::Exact(r) => self.exact.push(r),
        }
    }

    #[inline(always)]
    pub fn merge(&mut self, other: SearchResults) {
        self.exact.extend(other.exact);
        self.contains.extend(other.contains);
    }
}
