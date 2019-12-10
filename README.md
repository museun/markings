# markings

## Simple usage
```rust
use markings::{Args, Template, Opts};
// template strings are simply just ${key} markers in a string
// they are replaced with a cooresponding value when .apply() is used
let input = "hello ${name}, an answer: ${greeting}.";

// parse a template with the default options
// templates are clonable, they are 'consumed' on application.
let template = Template::parse(&input, Opts::default()).unwrap();

// construct some replacement args, this is reusable
let args = Args::new()
     // with constructs a key:val pair,
     // key must be a &str,
     // value is anything that implements std::fmt::Display
    .with("name", &"test-user")
    .with("greeting", &false)
    .build(); // construct the args

// apply the pre-computed args to the template, consuming the template
let output = template.apply(&args).unwrap();
assert_eq!(output, "hello test-user, an answer: false.");
```

License: 0BSD
