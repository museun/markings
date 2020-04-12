//! A simple string-based template 'language'
//!
//! This simply allows you to replace ${key} in a string with a 'Val' that imls. `std::fmt::Display`
//!
//! # Simple usage
//! ```
//! use markings::{Args, Template, Opts};
//! // template strings are simply just ${key} markers in a string
//! // they are replaced with a cooresponding value when .apply() is used
//! let input = "hello ${name}, an answer: ${greeting}.";
//!
//! // parse a template with the default options
//! // templates are clonable, they are 'consumed' on application.
//! let template = Template::parse(&input, Opts::default()).unwrap();
//!
//! // construct some replacement args, this is reusable
//! let args = Args::new()
//!      // with constructs a key:val pair,
//!      // key must be a &str,
//!      // value is anything that implements std::fmt::Display
//!     .with("name", &"test-user")
//!     .with("greeting", &false);
//!
//! // apply the pre-computed args to the template, consuming the template
//! let output = template.apply(&args).unwrap();
//! assert_eq!(output, "hello test-user, an answer: false.");
//! ```

use std::collections::HashMap;

/// An error produced by this crate
#[derive(Debug)]
pub enum Error {
    /// Mismatched braces were found
    ///
    /// `open` count and `closed` count
    MismatchedBraces { open: usize, close: usize },

    /// Expected a closing brace for open brace
    ///
    /// `head` is the offset for the nearest open brace
    ExpectedClosing { head: usize },

    /// Expected a opening brace for close brace
    ///
    /// `tail` is the offset for the nearest close brace
    ExpectedOpening { tail: usize },

    /// Nested template was found
    ///
    /// `pos` is where the template begins
    NestedTemplate { pos: usize },

    /// Duplicate keys were found, but not configured in [`Opts`](./struct.Opts.html)
    DuplicateKeys,

    /// An empty template was found, but not configured in [`Opts`](./struct.Opts.html)
    EmptyTemplate,

    /// Optional keys were found, but not configured in [`Opts`](./struct.Opts.html)
    OptionalKeys,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;
        match self {
            MismatchedBraces { open, close } => write!(
                f,
                "found {} open braces, and {} closed braces. a mistmatch",
                open, close
            ),
            ExpectedClosing { head } => write!(f, "expected closing bracket from offset {}", head),
            ExpectedOpening { tail } => write!(f, "expected opening bracket from offset {}", tail),
            NestedTemplate { pos } => write!(f, "nested template starting at offset: {}", pos),
            DuplicateKeys => f.write_str("duplicate keys were found"),
            EmptyTemplate => f.write_str("empty template was found"),
            OptionalKeys => f.write_str("optional keys were found"),
        }
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone)]
struct State<'a> {
    keys: Vec<&'a str>,
}

impl<'a> State<'a> {
    fn new(keys: Vec<&'a str>) -> Self {
        Self { keys }
    }

    fn has_keys(&self) -> bool {
        !self.keys.is_empty()
    }

    fn remove(&mut self, key: &str) -> Option<(&'a str, usize)> {
        if self.keys.is_empty() {
            return None;
        }

        let mut out = None;
        let mut i = 0;
        while i != self.keys.len() {
            if self.keys[i] == key {
                let s = self.keys.remove(i);
                let (_, count) = out.get_or_insert_with(|| (s, 0));
                *count += 1;
            } else {
                i += 1;
            }
        }
        out
    }

    fn has_duplicates(&self) -> bool {
        let mut set = std::collections::HashSet::new();
        self.keys.iter().any(|key| !set.insert(key))
    }
}

/// Templates allows for string replacement by **name**
///
/// ```
/// use markings::{Template, Args, Opts};
/// // parse a template using the default options
/// // the template is clonable so you don't have to reparse it
/// let template = Template::parse("hello, ${world}${end}", Opts::default())
///     .unwrap();
///
/// // build re-usable args that act as the replacements for the keys in the template
/// let args = Args::new()
///     .with("world", &"world")
///     .with("end", &(0x21 as char));
///
/// // apply the args to the template, consuming the template
/// let template = template
///     .apply(&args)
///     .unwrap();
///
/// // you'll get a String out, hopefully, that has your new message
/// assert_eq!(template, "hello, world!");
/// ```
/// See [`Template::apply`](./fn.Template.apply.html) for applying arguments to this template.
///
/// See [`Opts`](./struct.Opts.html) for a way to change the behavior of the parser
#[derive(Clone, Debug)]
pub struct Template<'a> {
    data: String, // total string
    state: State<'a>,
    opts: Opts,
}

impl<'a> Template<'a> {
    /// Parses a new template from a string
    ///
    /// The syntax is extremely basic: just `${key}`
    ///
    /// The *key* gets replaced by a *value* matching it during the [`Template::apply`](./struct.Template.html#method.apply) call
    pub fn parse(input: &'a str, opts: Opts) -> Result<Self, Error> {
        let state = State::new(Self::find_keys(input)?);
        opts.validate(&state)?;
        Ok(Self {
            data: input.to_string(),
            state,
            opts,
        })
    }

    /// Was this template empty?
    pub fn is_empty(&self) -> bool {
        self.opts.empty_template
    }

    /// Apply the arguments to the template
    ///
    /// One can use the [`Args`](./struct.Args.html) builder to make this less tedious
    pub fn apply<'k>(mut self, args: &Args<'k>) -> Result<String, Error> {
        for (key, val) in &args.mapping {
            let matches = self.state.remove(key);
            match matches {
                Some((match_, _)) => {
                    let s = self.data.replace(&format!("${{{}}}", match_), &val);
                    std::mem::replace(&mut self.data, s);
                }
                None if self.opts.optional_keys || self.is_empty() => continue,
                _ => return Err(Error::OptionalKeys),
            }
        }

        self.data.shrink_to_fit();
        Ok(self.data)
    }

    /// Find all the *keys* in the input string, returning them in a Vec
    ///
    /// This is exposed as a convenient function for doing pre-parsing.
    ///
    /// This returns an error if there are:
    /// * nested templates
    /// * mismatched braces
    ///
    /// ```
    /// # use markings::Template;
    /// let keys = Template::find_keys("${this} is a ${test} ${with some keys}").unwrap();
    /// assert_eq!(keys, vec!["this", "test", "with some keys"]);
    /// ```
    pub fn find_keys(input: &str) -> Result<Vec<&str>, Error> {
        let mut heads = vec![];
        let mut tails = vec![];

        let mut last = None;
        let mut iter = input.char_indices().peekable();
        while let Some((pos, ch)) = iter.next() {
            if ch == '$' && iter.peek().map(|&(_, d)| d == '{').unwrap_or_default() {
                last.replace(pos);
                heads.push(pos);
                iter.next();
            }
            if ch == '{' && last.is_some() {
                return Err(Error::NestedTemplate { pos });
            }

            if ch == '}' && last.is_some() {
                tails.push(pos);
                last.take();
            }
        }

        if heads.len() != tails.len() {
            return Err(Error::MismatchedBraces {
                open: heads.len(),
                close: tails.len(),
            });
        }

        tails.reverse();

        let mut keys = Vec::with_capacity(heads.len());
        for head in heads {
            let tail = tails.pop().ok_or_else(|| Error::ExpectedClosing { head })?;
            if tail > head {
                keys.push(&input[head + 2..tail]);
            } else {
                return Err(Error::ExpectedOpening { tail });
            }
        }

        if !tails.is_empty() {
            return Err(Error::MismatchedBraces {
                open: 0,
                close: tails.len(),
            });
        }

        Ok(keys)
    }
}

/// `Opts` are a set of options to configure how a template will be **parsed** and **applied**
///
/// ### The default options would fail if
/// - there is an empty template (e.g. no replacement keys)
/// - there are duplicate keys
/// - apply will fail if the exact keys aren't applied
///
/// ## default options
/// ```
/// # use markings::{Template, Opts};
/// let input = "this is a ${name}.";
/// let template = Template::parse(&input, Opts::default()).unwrap();
/// ```
/// ## various options
/// ```
/// # use markings::{Template, Opts};
/// // this will allow these options in the parsing/application
/// let opts = Opts::default()
///     .optional_keys()  // optional keys -- args aren't required to match the template keys
///     .duplicate_keys() // duplicate keys -- duplicate keys in the template will use the same argument
///     .empty_template() // templates can just be strings that act as an "identity"
///     .build();
///
/// let input = "this is a ${name}.";
/// let template = Template::parse(&input, opts).unwrap();
#[derive(Default, Copy, Clone, Debug, PartialEq)]
pub struct Opts {
    optional_keys: bool,
    duplicate_keys: bool,
    empty_template: bool,
}

impl Opts {
    /// Allow optional keys
    ///
    /// Keys found in the template application don't have to appear in the template
    pub fn optional_keys(&mut self) -> &mut Self {
        self.optional_keys = !self.optional_keys;
        self
    }

    /// Allow duplicate keys
    ///
    /// Multiple keys in the template will be replaced by the same argument
    pub fn duplicate_keys(&mut self) -> &mut Self {
        self.duplicate_keys = !self.duplicate_keys;
        self
    }

    /// Allows for an empty template -- e.g. a template without any args
    ///
    /// When args are applied to this, the original string is returned
    pub fn empty_template(&mut self) -> &mut Self {
        self.empty_template = !self.empty_template;
        self
    }

    /// Construct the option set
    pub fn build(self) -> Self {
        self
    }

    fn validate(self, keys: &State<'_>) -> Result<(), Error> {
        if !self.empty_template && !keys.has_keys() {
            return Err(Error::EmptyTemplate);
        }
        if !self.duplicate_keys && keys.has_duplicates() {
            return Err(Error::DuplicateKeys);
        }
        Ok(())
    }
}

/// This is an easy way to build an argument mapping for the [`template application`](./struct.Template.html#method.apply) method
///
/// The *key* must be a [`&str`](https://doc.rust-lang.org/std/primitive.str.html) while the *value* can be any [`std::fmt::Display`](https://doc.rust-lang.org/std/path/struct.Display.html) trait object
///
/// **note** The keys are unique, duplicates will be replaced by the last one
/// ```
/// # use markings::Args;
/// let args = Args::new()
///     .with("key1", &false)
///     .with("key2", &"message")
///     .with("key3", &41)
///     .with("key3", &42);
/// # assert_eq!(args.len(), 3)
/// ```
#[derive(Default, Clone)]
pub struct Args<'k> {
    mapping: HashMap<std::borrow::Cow<'k, str>, String>,
}

impl<'k> Args<'k> {
    /// Create a new Args builder
    pub fn new() -> Self {
        Self {
            mapping: HashMap::new(),
        }
    }

    /// Length of the args
    pub fn len(&self) -> usize {
        self.mapping.len()
    }

    /// Whether the args is empty
    pub fn is_empty(&self) -> bool {
        self.mapping.is_empty()
    }

    /// Maps a key to a type that implements [`std::fmt::Display`](https://doc.rust-lang.org/std/fmt/trait.Display.html)
    pub fn with(
        mut self,
        key: impl Into<std::borrow::Cow<'k, str>>,
        val: impl std::fmt::Display,
    ) -> Self {
        self.mapping.insert(key.into(), val.to_string().into());
        self
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'_ std::borrow::Cow<'k, str>, &'_ String)> + '_ {
        self.mapping.iter()
    }
}

pub type ArgsIntoIter<'k> = std::collections::hash_map::IntoIter<std::borrow::Cow<'k, str>, String>;

impl<'k> IntoIterator for Args<'k> {
    type Item = (std::borrow::Cow<'k, str>, String);
    type IntoIter = ArgsIntoIter<'k>;
    fn into_iter(self) -> Self::IntoIter {
        self.mapping.into_iter()
    }
}

impl<'k, K, V> std::iter::FromIterator<(K, V)> for Args<'k>
where
    K: Into<std::borrow::Cow<'k, str>>,
    V: std::fmt::Display,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        Self {
            mapping: iter
                .into_iter()
                .map(|(k, v)| (k.into(), v.to_string()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_key() {
        let args = Args::new()
            .with("a", &true)
            .with("a", &false)
            .with("a", &true);

        let v = args
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect::<Vec<_>>();
        assert_eq!(v, vec![("a".into(), "true".to_string())]);
    }

    #[test]
    fn duplicates() {
        let state = State::new(vec!["a", "b", "c"]);
        assert!(!state.has_duplicates());

        let state = State::new(vec!["a", "b", "a", "c"]);
        assert!(state.has_duplicates());
    }

    #[test]
    fn basic() {
        let p = Template::parse("${a} ${b}${c}", Default::default()).unwrap();
        let a = Args::new().with("a", &0).with("b", &1).with("c", &2);
        let t = p.apply(&a).unwrap();
        assert_eq!(t, "0 12");
    }

    #[test]
    fn apply_iter() {
        let mut base = (b'a'..=b'z')
            .map(|c| format!("${{{}}}", c as char))
            .collect::<Vec<_>>()
            .join(" ");

        for c in b'a'..=b'z' {
            let t = Template::parse(&base, Default::default()).unwrap();
            let a = Args::new().with(format!("{}", c as char), format!("{} = {}", c as char, c));
            base = t.apply(&a).unwrap();
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
    fn owned_key() {
        let args: Args<'static> = Args::new().with("foo".to_string(), 42);
        assert_eq!(args.len(), 1);
    }

    #[test]
    fn with_args() {
        let template = "you've reached a max of ${max} credits, \
                        out of ${total} total credits with ${success} \
                        successes and ${failure} failures. and I've \
                        'collected' ${overall_total} credits from all of \
                        the failures.";

        let t = Template::parse(&template, Default::default()).unwrap();
        let parts = Args::new()
            .with("max", &"218,731")
            .with("total", &"706,917")
            .with("success", &"169")
            .with("failure", &"174")
            .with("overall_total", &"1,629,011");

        let expected = "you've reached a max of 218,731 credits, \
                        out of 706,917 total credits with 169 \
                        successes and 174 failures. and I've \
                        'collected' 1,629,011 credits from all of \
                        the failures.";

        assert_eq!(t.apply(&parts).unwrap(), expected);
    }

    #[test]
    fn empty_template() {
        let input = "";
        Template::parse(&input, Default::default()).unwrap_err(); // TODO assert this error

        let template = Template::parse(&input, Opts::default().empty_template().build()).unwrap();
        assert!(template.is_empty());
        assert_eq!(input, template.apply(&Args::new()).unwrap());

        let input = "foobar baz quux {{something}}";
        Template::parse(&input, Default::default()).unwrap_err(); // TODO assert this error

        let template = Template::parse(&input, Opts::default().empty_template().build()).unwrap();
        assert!(template.is_empty());
        assert_eq!(input, template.apply(&Args::new()).unwrap());
    }

    #[test]
    fn duplicate_keys() {
        let input = "${one} and ${two} and ${one}";
        Template::parse(&input, Default::default()).unwrap_err(); //TODO assert this error

        let input = "${one} and ${two} and ${one}";
        let template = Template::parse(&input, Opts::default().duplicate_keys().build()).unwrap();
        let parts = Args::new().with("one", &1).with("two", &2);
        assert_eq!("1 and 2 and 1", template.apply(&parts).unwrap());
    }

    #[test]
    fn optional_keys() {
        let input = "${foo} ${bar} ${baz}";

        let parts = Args::new().with("foo", &false).with("unknown", &true);

        let template = Template::parse(&input, Default::default()).unwrap();
        template.apply(&parts).unwrap_err(); // TODO assert this error

        let template = Template::parse(&input, Opts::default().optional_keys().build()).unwrap();
        assert_eq!("false ${bar} ${baz}", template.apply(&parts).unwrap());
    }

    #[test]
    fn empty_template_replace() {
        let template =
            Template::parse("${short_name}", Opts::default().empty_template().build()).unwrap();
        let parts = Args::new().with("short_name", &1);
        assert_eq!("1", template.apply(&parts).unwrap());
    }

    #[test]
    fn args_owned() {
        let args = Args::new().with("foo", 42).with("bar", false);
        let template = Template::parse("${foo} ${bar}", Default::default()).unwrap();
        let s = template.apply(&args).unwrap();
        assert_eq!(s, "42 false");
    }
}
