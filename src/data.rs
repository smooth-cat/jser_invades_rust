#[derive(Debug)]
pub struct Person {
  pub name: String,
  pub age: i32,
}

pub struct WriteWrap<'a> {
  pub write: &'a mut String,
}