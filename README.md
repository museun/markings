# markings
a extremely simple template "language"

optional features: "hashbrown" to enable faster hashmaps with lower allocation counts

usage:
```rust
use markings::{Args, Template};
struct Foo<'a> { data: &'a str }
impl<'a> std::fmt::Display for Foo<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.data)
    }
}

#[derive(Debug)]
struct Bar { ch: u8 }

let foo = Foo { data: "template" };
let bar = format!("{}", Bar { ch: b'!' }.ch as char);
let uses = format!("{:#X} uses", 42 + 7);

let input = "this is a ${string} with ${replacements}${end}";
let template = Template::parse(&input).expect("well formed template");
let args = Args::new()
    .with("string", &foo)
    .with("replacements", &uses)
    .with("end", &bar)
    .build();
let output = template.apply(&args).expect("args should be correct");

println!("{}", output); // => "this is a template with 0x31 uses!"
```
