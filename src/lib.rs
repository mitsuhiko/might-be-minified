//! A crate that helps to decide if something is a minified JavaScript
//! file.

use std::cmp::PartialOrd;
use std::io::Read;

use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref BASIC_TOKEN_RE: Regex = Regex::new(
        &r#"(?mx)
        (?P<comment>
            //.*?$ |
            /\*.*?\*/) |
        (?P<whitespace>
            \s+) |
        (?P<string>
            '([^'\\]*(?:\\.[^'\\]*)*)' |
            "([^"\\]*(?:\\.[^"\\]*)*)" |
            `([^`\\]*(?:\\.[^`\\]*)*)`) |
        (?P<regex_op>
            \\/|/) |
        (?P<keyword>(?:\b
            async|
            await|
            break|
            case|
            catch|
            class|
            continue|
            debugger|
            default|
            delete|
            do|
            else|
            export|
            extends|
            finally|
            for|
            function|
            if|
            import|
            instanceof|
            in|
            let|
            new|
            null|
            return|
            static|
            super|
            switch|
            this|
            throw|
            true|
            try|
            typeof|
            var|
            void|
            while|
            with)\b) |
        (?P<ident>
            [\p{Lu}\p{Ll}\p{Lt}\p{Lm}\p{Lo}\p{Nl}$_]
            [\p{Lu}\p{Ll}\p{Lt}\p{Lm}\p{Lo}\p{Nl}\p{Mn}\p{Mc}\p{Nd}\p{Pc}$_]*
        )
    "#
    )
    .unwrap();
}
lazy_static! {
    static ref IDENT_RE: Regex = Regex::new(
        &r#"(?mx)
        [\p{Lu}\p{Ll}\p{Lt}\p{Lm}\p{Lo}\p{Nl}$_]
        [\p{Lu}\p{Ll}\p{Lt}\p{Lm}\p{Lo}\p{Nl}\p{Mn}\p{Mc}\p{Nd}\p{Pc}$_]*
    "#
    )
    .unwrap();
}

fn partial_clamp<T: PartialOrd>(l: T, u: T, v: T) -> T {
    if v < l {
        l
    } else if v > u {
        u
    } else {
        v
    }
}

/// Provides an analysis of a source file
pub struct Analysis {
    line_lengths: Vec<usize>,
    ident_lengths: Vec<usize>,
    space_count: usize,
    non_space_count: usize,
}

/// Analyze JavaScript contained in a script
///
/// Example:
///
/// ```rust
/// # use might_be_minified::analyze_str;
/// if analyze_str("...").is_likely_minified() {
///     println!("This is probably a minified file");
/// }
/// ```
pub fn analyze_str(code: &str) -> Analysis {
    let mut line_lengths = vec![];
    let mut ident_lengths = vec![];
    let mut space = 0;
    let mut not_space = 1;
    let mut line_width = 0;

    // do basic counting on layout and spaces.
    for c in code.chars() {
        // count whitespace
        if c == '\t' {
            space += 4;
        } else if c.is_whitespace() {
            space += 1;
        } else {
            not_space += 1;
        }

        // detect shapes
        if c == '\r' {
            continue;
        } else if c == '\n' {
            if line_width > 0 {
                line_lengths.push(line_width);
            }
            line_width = 0;
        } else {
            line_width += if c == '\t' { 4 } else { 1 };
        }
    }
    if line_width > 0 {
        line_lengths.push(line_width);
    }

    // shitty tokenization.  This is known to be broken but it's "good enough"
    // to do a basic detection on if this is javascript or not.  In particular
    // we count keywords and a name length histogram.
    for m in BASIC_TOKEN_RE.find_iter(code) {
        if IDENT_RE.is_match(m.as_str()) {
            ident_lengths.push(m.end() - m.start());
        }
    }

    line_lengths.sort();
    ident_lengths.sort();

    Analysis {
        line_lengths: line_lengths,
        ident_lengths: ident_lengths,
        space_count: space,
        non_space_count: not_space,
    }
}

/// Analyze JavaScript behind a reader
pub fn analyze<R: Read>(mut rdr: R) -> Analysis {
    let mut rv = String::new();
    rdr.read_to_string(&mut rv).unwrap();
    analyze_str(&rv)
}

impl Analysis {
    /// Returns the whitespace to code ratio
    ///
    /// This is a useful metric to decide on if a file is likely minified code
    /// or regular JavaScript code.
    pub fn space_to_code_ratio(&self) -> f32 {
        self.space_count as f32 / self.non_space_count as f32
    }

    /// The median identifier length
    ///
    /// This returns the median length for an identifier (name) in the JS
    /// source code.
    pub fn median_ident_length(&self) -> usize {
        if self.line_lengths.is_empty() {
            0
        } else {
            *self
                .ident_lengths
                .get(self.ident_lengths.len() / 2)
                .unwrap_or(&0)
        }
    }

    /// The longest code line length
    ///
    /// This returns the length of the longest line.  This includes comments
    /// and other things.
    pub fn longest_line(&self) -> usize {
        if self.line_lengths.is_empty() {
            0
        } else {
            *self
                .line_lengths
                .get(self.line_lengths.len() - 1)
                .unwrap_or(&0)
        }
    }

    /// The "shape" of the code file
    ///
    /// This essentially is `height / width` of the file where the height is
    /// the actual number of lines however the width is the p75 line length.
    pub fn shape(&self) -> f32 {
        if self.line_lengths.is_empty() {
            return 0.0;
        }
        let width = self.line_lengths[self.line_lengths.len() / 4 * 3];
        let height = self.line_lengths.len();
        height as f32 / width as f32
    }

    /// The proability of the file being minified
    ///
    /// Effectively 1.0 (which is unlikely to be reached) means the file is
    /// definitely minified.  Anything above 0.5 is considered likely to be
    /// minified.
    pub fn minified_probability(&self) -> f32 {
        let p_space = (0.5 - partial_clamp(0.0, 0.5, self.space_to_code_ratio())) * 2.0;
        let p_name = (5 - (partial_clamp(1, 6, self.median_ident_length()) - 1)) as f32 / 5.0;
        let p_shape = (20.0 - partial_clamp(0.0, 20.0, self.shape())) / 20.0;
        let p_line = partial_clamp(0, 1000, self.longest_line()) as f32 / 1000.0;
        (p_space * 0.1 + p_name * 0.4 + p_shape * 0.2 + p_line * 0.3)
    }

    /// Indicates that the file is likely minified
    pub fn is_likely_minified(&self) -> bool {
        self.minified_probability() > 0.5
    }
}
