//! ```
//! # fn main() {
//! use markings::{Args, Template};
//! struct Foo<'a> {
//!     thing: &'a str,
//! }
//! impl<'a> std::fmt::Display for Foo<'a> {
//!     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//!         write!(f, "{}", self.thing)
//!     }
//! }
//!
//! let test = Foo{thing: "test"};
//! let test = Args::new().with("thing", &test).with("end", &"!").build();
//!
//! let demo = Foo{thing: "demo_"};
//! let demo = Args::new().with("thing", &demo).with("end", &42).build();
//!
//! let input = "this is a ${thing}${end}";
//! let template = Template::parse(&input).unwrap();
//!
//! let output = template.clone().apply(&test).unwrap();
//! assert_eq!(output, "this is a test!");
//!
//! let output = template.clone().apply(&demo).unwrap();
//! assert_eq!(output, "this is a demo_42");
//! # }
//! ```

use std::collections::HashMap;
use std::ops::RangeInclusive;

/// Template allows for string replacement by name
///
/// ```
/// # use markings::{Template, Args};
///
/// let world = "world";
/// let message = Template::parse("hello, ${world}!")
///     .unwrap()
///     .apply(&Args::new().with("world", &world).build())
///     .unwrap();
/// println!("{}", message); // => hello, world!
///
/// # assert_eq!(message, "hello, world!");
/// ```
#[derive(Clone, Debug)]
pub struct Template {
    data: String,                 // total string
    left: String,                 // left most part
    index: RangeInclusive<usize>, // index of left most part
    is_identity: bool, // whether this template is just an identity template (e.g. a no-op)
}

impl Template {
    /// Find all of the keys in the template, returning their names
    ///
    /// This will return an error on:
    /// * Duplicate keys
    /// * Unsupported templates (e.g. nested)
    /// * Empty Templates (so you can skip parsing/application)
    /// * Failure of the `x` cmp function
    /// * Failure of the `xs` cmp function
    ///
    /// See [`KeyError`](./enum.KeyError.html) for errors this can return
    ///
    /// `x` is a comparison function that is applied to the first character of each key
    ///
    /// `xs` is a comparison function that is applied to the rest of the key
    pub fn find_keys(
        input: &str,
        x: fn(&char) -> bool,
        xs: fn(&char) -> bool,
    ) -> Result<Vec<&str>, KeyError<'_>> {
        let mut set = Vec::new();
        let mut start = None;
        let mut iter = input.char_indices().peekable();
        loop {
            match (iter.next(), iter.peek(), start) {
                (Some((i, '$')), Some((_, '{')), None) => {
                    start.replace(i);
                }
                (Some((_, '$')), Some((_, '{')), Some(s)) => {
                    return Err(KeyError::NotSupported("nested templates", s..=input.len()));
                }
                (Some((i, '}')), _, Some(s)) => {
                    let t = input[s + 2..i].trim();
                    if t.is_empty() {
                        return Err(KeyError::EmptyTemplate(s..=i));
                    }
                    if !t.chars().next().as_ref().map(x).unwrap() {
                        return Err(KeyError::InvalidKeyStart(t, s..=i));
                    }
                    if !t.chars().all(|c| xs(&c)) {
                        return Err(KeyError::InvalidKey(t, s..=i));
                    }
                    if set.contains(&t) {
                        return Err(KeyError::DuplicateKey(t, s..=i));
                    }
                    set.push(t);
                    start.take();
                }
                (Some((_, ..)), ..) => continue,
                (None, ..) => break,
            }
        }

        Ok(set)
    }

    pub fn is_empty(&self) -> bool {
        self.is_identity
    }

    /// Parses a new template from a string
    ///
    /// The syntax is extremely basic: just `${key}`
    ///
    /// It gets replaced by a value matching the key during the [`Template::apply`](./struct.Template.html#method.apply) call
    pub fn parse(input: &str) -> Result<Self, Error> {
        let mut iter = input.char_indices().peekable();

        let mut start = None;
        while let Some((i, ch)) = iter.next() {
            // TODO: this doesn't balance the brackets.. oops
            if let ('$', Some((_, '{'))) = (ch, iter.peek()) {
                start.replace(i);
            }
            if let ('}', Some(n)) = (ch, start) {
                return Ok(Self {
                    left: input[n + 2..i].into(),
                    index: RangeInclusive::new(n, i),
                    data: input.into(),
                    is_identity: false,
                });
            }
        }

        match start {
            Some(n) => Err(Error::Unbalanced(n)),
            // no template was found, so this is an "identity" template
            None => Ok(Self {
                left: "".into(),
                data: input.into(),
                index: RangeInclusive::new(0, input.len()),
                is_identity: true,
            }),
        }
    }

    /// Apply the arguments to the template
    ///
    /// One can use the [`Args`](./struct.Args.html) builder to make this less tedious
    pub fn apply<'repr, I, V>(mut self, parts: I) -> Result<String, Error>
    where
        I: IntoIterator<Item = &'repr (&'repr str, V)> + 'repr,
        V: std::fmt::Display + 'repr,
    {
        // don't bother apply args and return the input string
        if self.is_identity {
            return Ok(self.data);
        }

        let parts = parts
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect::<HashMap<_, _>>(); // this order doesn't matter

        debug_assert!(!parts.is_empty());

        let mut seen = 0;
        while seen < parts.len() {
            let part = match parts.get(&self.left.as_str()) {
                Some(part) => part,
                None => return Err(Error::Missing(self.left)),
            };
            self.data.replace_range(self.index.clone(), &part);
            if seen == parts.len() - 1 {
                break;
            }

            let this = match Self::parse(&self.data) {
                Err(Error::EmptyTemplate) => break,
                Err(err) => return Err(err),
                Ok(this) => this,
            };
            std::mem::replace(&mut self, this);
            seen += 1;
        }

        let mut data = self.data.to_string();
        data.shrink_to_fit();
        Ok(data)
    }
}

/// This allows you to be args for the [`Template::apply`](./struct.Template.html#method.apply) method
///
/// ```
/// # use markings::Args;
/// let args = Args::new()
///                 .with("key1", &false)
///                 .with("key2", &"message")
///                 .with("key3", &42)
///                 .build();
/// # assert_eq!(args.len(), 3)
/// ```
pub struct Args<'a>(HashMap<&'a str, &'a dyn std::fmt::Display>);

impl<'a> Default for Args<'a> {
    /// Create a new Args builder
    fn default() -> Self {
        Self(HashMap::new())
    }
}

impl<'a> Args<'a> {
    /// Create a new Args builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Maps a key to a type that implements [`std::fmt::Display`](https://doc.rust-lang.org/std/fmt/trait.Display.html)
    pub fn with(mut self, key: &'a str, val: &'a dyn std::fmt::Display) -> Self {
        self.0.insert(key, val);
        self
    }

    /// Completes the builder, returning a Vec of Key : Values
    pub fn build(self) -> Vec<(&'a str, &'a dyn std::fmt::Display)> {
        self.0.into_iter().collect()
    }
}

/// Errors returned by the Template parser/applier
#[derive(Debug, PartialEq)]
pub enum Error {
    /// The template is empty
    EmptyTemplate,
    /// Unbalanced at `pos`: every ${ needs to be paired with a }
    Unbalanced(usize),
    /// The `key` is missing
    Missing(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::EmptyTemplate => write!(f, "template contains no replacement strings"),
            Error::Unbalanced(start) => write!(f, "unbalanced bracket starting at: {}", start),
            Error::Missing(key) => write!(f, "template key '{}' is missing", key),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, PartialEq)]
pub enum KeyError<'a> {
    NotSupported(&'a str, RangeInclusive<usize>),
    EmptyTemplate(RangeInclusive<usize>),
    InvalidKeyStart(&'a str, RangeInclusive<usize>),
    InvalidKey(&'a str, RangeInclusive<usize>),
    DuplicateKey(&'a str, RangeInclusive<usize>),
}

impl<'a> std::fmt::Display for KeyError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use KeyError::*;
        match self {
            NotSupported(msg, range) => {
                write!(f, "not supported: {} at pos: {:?}", msg, range)?;
            }
            EmptyTemplate(range) => {
                write!(f, "empty template at pos: {:?}", range)?;
            }
            InvalidKeyStart(name, range) => {
                write!(
                    f,
                    "invalid name `{}` must start with A-Za-z at pos: {:?}",
                    name, range
                )?;
            }
            InvalidKey(name, range) => {
                write!(f, "invalid name: `{}` at pos: {:?}", name, range)?;
            }
            DuplicateKey(name, range) => {
                write!(f, "duplicate name: `{}` at pos: {:?}", name, range)?;
            }
        };
        Ok(())
    }
}

impl<'a> std::error::Error for KeyError<'a> {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic() {
        let p = Template::parse("${a} ${b}${c}").unwrap();
        let t = p.apply(&[("a", &0), ("b", &1), ("c", &2)]).unwrap();
        assert_eq!(t, "0 12");
    }

    #[test]
    fn apply_iter() {
        let mut base = (b'a'..=b'z')
            .map(|c| format!("${{{}}}", c as char))
            .collect::<Vec<_>>()
            .join(" ");

        for c in b'a'..=b'z' {
            let t = Template::parse(&base).unwrap();
            base = t
                .apply(&[(
                    format!("{}", c as char).as_ref(),
                    format!("{} = {}", c as char, c),
                )])
                .unwrap();
        }

        let expected = "a = 97 b = 98 c = 99 d = 100 e = 101 \
                        f = 102 g = 103 h = 104 i = 105 j = 106 \
                        k = 107 l = 108 m = 109 n = 110 o = 111 \
                        p = 112 q = 113 r = 114 s = 115 t = 116 \
                        u = 117 v = 118 w = 119 x = 120 y = 121 \
                        z = 122";

        assert_eq!(base, expected);
    }

    #[test]
    fn real_template() {
        let template = "you've reached a max of ${max} credits, \
                        out of ${total} total credits with ${success} \
                        successes and ${failure} failures. and I've \
                        'collected' ${overall_total} credits from all of \
                        the failures.";

        let t = Template::parse(&template).unwrap();
        let out = t
            .apply(&[
                ("max", &"218,731"),
                ("total", &"706,917"),
                ("success", &"169"),
                ("failure", &"174"),
                ("overall_total", &"1,629,011"),
            ])
            .unwrap();

        let expected = "you've reached a max of 218,731 credits, \
                        out of 706,917 total credits with 169 \
                        successes and 174 failures. and I've \
                        'collected' 1,629,011 credits from all of \
                        the failures.";
        assert_eq!(out, expected);
    }

    #[test]
    fn with_args() {
        let template = "you've reached a max of ${max} credits, \
                        out of ${total} total credits with ${success} \
                        successes and ${failure} failures. and I've \
                        'collected' ${overall_total} credits from all of \
                        the failures.";

        let t = Template::parse(&template).unwrap();
        let parts = Args::new()
            .with("max", &"218,731")
            .with("total", &"706,917")
            .with("success", &"169")
            .with("failure", &"174")
            .with("overall_total", &"1,629,011")
            .build();

        let expected = "you've reached a max of 218,731 credits, \
                        out of 706,917 total credits with 169 \
                        successes and 174 failures. and I've \
                        'collected' 1,629,011 credits from all of \
                        the failures.";

        assert_eq!(t.apply(&parts).unwrap(), expected);
    }

    #[test]
    fn identity_string() {
        let input = "foobar baz quux {{something}}";
        let template = Template::parse(&input).unwrap();
        assert!(template.is_empty());
        let parts = Args::new().build();
        assert_eq!(input, template.apply(&parts).unwrap());
    }

    #[test]
    fn find_keys() {
        let input = "${test} ${foo} ${bar}";
        let list =
            Template::find_keys(&input, char::is_ascii_alphabetic, char::is_ascii_alphabetic)
                .unwrap();
        assert_eq!(list, vec!["test", "foo", "bar"]);

        let input = "${a} ${b} ${b}";
        let err = Template::find_keys(&input, char::is_ascii_alphabetic, char::is_ascii_alphabetic)
            .unwrap_err();
        assert_eq!(err, KeyError::DuplicateKey("b", 10..=13));

        let input = "${a} ${} ${b}";
        let err = Template::find_keys(&input, char::is_ascii_alphabetic, char::is_ascii_alphabetic)
            .unwrap_err();
        assert_eq!(err, KeyError::EmptyTemplate(5..=7));

        let input = "${a} ${${asdf}}";
        let err = Template::find_keys(&input, char::is_ascii_alphabetic, char::is_ascii_alphabetic)
            .unwrap_err();
        assert_eq!(
            err,
            KeyError::NotSupported("nested templates", 5..=input.len())
        );

        let input = "${good} ${_bad}";
        let err = Template::find_keys(&input, char::is_ascii_alphabetic, char::is_ascii_alphabetic)
            .unwrap_err();
        assert_eq!(err, KeyError::InvalidKeyStart("_bad", 8..=14));

        let input = "${good} ${b_ad}";
        let err = Template::find_keys(&input, char::is_ascii_alphabetic, char::is_ascii_alphabetic)
            .unwrap_err();
        assert_eq!(err, KeyError::InvalidKey("b_ad", 8..=14));
    }
}
