use regex::Regex;

#[derive(Debug)]
pub struct ColumnsFilter {
    /// Regex pattern if filter was created via regex; None for manual hide/show.
    pattern: Option<Regex>,
    indices: Vec<usize>,
    filtered_headers: Vec<String>,
    filtered_flags: Vec<bool>,
    num_columns_before_filter: usize,
    disabled_because_no_match: bool,
}

impl ColumnsFilter {
    pub fn new(pattern: Regex, headers: &[String]) -> Self {
        let mut indices = vec![];
        let mut filtered_headers: Vec<String> = vec![];
        let mut filtered_flags: Vec<bool> = vec![];
        for (i, header) in headers.iter().enumerate() {
            if pattern.is_match(header) {
                indices.push(i);
                filtered_headers.push(header.clone());
                filtered_flags.push(true);
            } else {
                filtered_flags.push(false);
            }
        }
        let disabled_because_no_match;
        if indices.is_empty() {
            indices = (0..headers.len()).collect();
            filtered_headers = headers.into();
            disabled_because_no_match = true;
        } else {
            disabled_because_no_match = false;
        }
        Self {
            pattern: Some(pattern),
            indices,
            filtered_headers,
            filtered_flags,
            num_columns_before_filter: headers.len(),
            disabled_because_no_match,
        }
    }

    /// Create a filter from an explicit list of origin column indices (must be sorted).
    pub fn from_indices(indices: Vec<usize>, headers: &[String]) -> Self {
        let mut filtered_flags = vec![false; headers.len()];
        let mut filtered_headers = Vec::with_capacity(indices.len());
        let mut unique_sorted = indices;
        unique_sorted.sort_unstable();
        unique_sorted.dedup();
        unique_sorted.retain(|&i| i < headers.len());
        for &i in &unique_sorted {
            filtered_flags[i] = true;
            filtered_headers.push(headers[i].clone());
        }
        let disabled_because_no_match = unique_sorted.is_empty();
        let indices = if disabled_because_no_match {
            filtered_headers = headers.into();
            (0..headers.len()).collect()
        } else {
            unique_sorted
        };
        Self {
            pattern: None,
            indices,
            filtered_headers,
            filtered_flags,
            num_columns_before_filter: headers.len(),
            disabled_because_no_match,
        }
    }

    /// Visible origin indices for the current filter (ignoring disabled-because-no-match).
    pub fn visible_origin_indices(&self, headers_len: usize) -> Vec<usize> {
        if self.disabled_because_no_match {
            (0..headers_len).collect()
        } else {
            self.indices.clone()
        }
    }

    /// Toggle visibility of `origin_index`. Returns None if nothing changed (e.g. only column).
    pub fn toggle_origin_index(
        origin_index: usize,
        headers: &[String],
        current: Option<&ColumnsFilter>,
    ) -> Option<Self> {
        if origin_index >= headers.len() {
            return None;
        }
        let mut visible = match current {
            Some(cf) => cf.visible_origin_indices(headers.len()),
            None => (0..headers.len()).collect(),
        };
        if let Some(pos) = visible.iter().position(|&i| i == origin_index) {
            if visible.len() <= 1 {
                return None;
            }
            visible.remove(pos);
        } else {
            visible.push(origin_index);
            visible.sort_unstable();
        }
        if visible.len() == headers.len() {
            // Represent "show all" by returning a special empty sentinel via disabled — callers
            // should reset the filter entirely when all columns are visible.
            return Some(Self::from_indices(visible, headers));
        }
        Some(Self::from_indices(visible, headers))
    }

    /// Keep only the given origin indices visible.
    pub fn only_indices(indices: Vec<usize>, headers: &[String]) -> Self {
        Self::from_indices(indices, headers)
    }

    pub fn filtered_headers(&self) -> &Vec<String> {
        &self.filtered_headers
    }

    pub fn indices(&self) -> &Vec<usize> {
        &self.indices
    }

    pub fn pattern(&self) -> Option<Regex> {
        self.pattern.clone()
    }

    /// Status label for the filter (regex pattern or a short description for manual filters).
    pub fn status_label(&self) -> String {
        if let Some(pattern) = &self.pattern {
            pattern.to_string()
        } else {
            "manual".to_string()
        }
    }

    pub fn num_filtered(&self) -> usize {
        self.indices.len()
    }

    pub fn num_original(&self) -> usize {
        self.num_columns_before_filter
    }

    pub fn disabled_because_no_match(&self) -> bool {
        self.disabled_because_no_match
    }

    pub fn is_column_filtered(&self, index: usize) -> bool {
        self.filtered_flags.get(index).cloned().unwrap_or(false)
    }

    /// Whether every column is visible (filter is a no-op).
    pub fn shows_all_columns(&self) -> bool {
        self.disabled_because_no_match || self.indices.len() == self.num_columns_before_filter
    }
}

/// Find the best matching column index for `query` among `headers`.
/// Priority: exact (case-insensitive) > prefix > contains > subsequence.
/// On ties at the same priority, the earliest column wins.
pub fn fuzzy_match_column(query: &str, headers: &[String]) -> Option<usize> {
    let query_lower = query.to_lowercase();
    if query_lower.is_empty() {
        return None;
    }

    let mut best: Option<(usize, u8)> = None;
    for (i, header) in headers.iter().enumerate() {
        let h = header.to_lowercase();
        let priority = if h == query_lower {
            0u8
        } else if h.starts_with(&query_lower) {
            1
        } else if h.contains(&query_lower) {
            2
        } else if is_subsequence(&query_lower, &h) {
            3
        } else {
            continue;
        };
        match best {
            None => best = Some((i, priority)),
            Some((_, bp)) if priority < bp => best = Some((i, priority)),
            _ => {}
        }
    }
    best.map(|(i, _)| i)
}

fn is_subsequence(query: &str, text: &str) -> bool {
    let mut text_chars = text.chars();
    for qc in query.chars() {
        loop {
            match text_chars.next() {
                Some(tc) if tc == qc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match_priority() {
        let headers = vec![
            "Latitude".to_string(),
            "Lat".to_string(),
            "City".to_string(),
            "lon_lat".to_string(),
        ];
        assert_eq!(fuzzy_match_column("Lat", &headers), Some(1)); // exact over prefix
        assert_eq!(fuzzy_match_column("lat", &headers), Some(1));
        assert_eq!(fuzzy_match_column("Lati", &headers), Some(0)); // prefix
        assert_eq!(fuzzy_match_column("ity", &headers), Some(2)); // contains
        assert_eq!(fuzzy_match_column("cty", &headers), Some(2)); // subsequence City
        assert_eq!(fuzzy_match_column("zzz", &headers), None);
        assert_eq!(fuzzy_match_column("", &headers), None);
    }

    #[test]
    fn test_from_indices_and_toggle() {
        let headers: Vec<String> = ["A", "B", "C", "D"].iter().map(|s| s.to_string()).collect();
        let cf = ColumnsFilter::from_indices(vec![0, 2], &headers);
        assert_eq!(cf.indices(), &vec![0, 2]);
        assert_eq!(
            cf.filtered_headers(),
            &vec!["A".to_string(), "C".to_string()]
        );
        assert!(cf.pattern().is_none());
        assert!(!cf.is_column_filtered(1));
        assert!(cf.is_column_filtered(0));

        // Hide B from full view
        let toggled = ColumnsFilter::toggle_origin_index(1, &headers, None).unwrap();
        assert_eq!(toggled.indices(), &vec![0, 2, 3]);

        // Show B again
        let shown = ColumnsFilter::toggle_origin_index(1, &headers, Some(&toggled)).unwrap();
        assert!(shown.shows_all_columns());

        // Cannot hide last column
        let only_a = ColumnsFilter::from_indices(vec![0], &headers);
        assert!(ColumnsFilter::toggle_origin_index(0, &headers, Some(&only_a)).is_none());
    }

    #[test]
    fn test_regex_pattern_still_works() {
        let headers: Vec<String> = ["City", "Lat", "Lon"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let cf = ColumnsFilter::new(Regex::new("(?i)lat|lon").unwrap(), &headers);
        assert_eq!(cf.indices(), &vec![1, 2]);
        assert!(cf.pattern().is_some());
        assert_eq!(cf.status_label(), "(?i)lat|lon");
    }
}
